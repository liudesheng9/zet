# Tree Lock: published PRD and issue coverage

Status: published and independently read back from GitHub on 2026-10-21. The user approved 18 focused AFK issues and the existing public/isolated test boundaries before publication.

Source: [implementation design](tree-lock-implementation-design.md), SHA-256 `fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a`. Preserve D1–D7. This document describes tracker planning, not implementation.

Parent: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123).

## Publication verification

- Fetched all 19 live titles, bodies, states and labels and matched the exact reviewed publication bodies after line-ending normalization.
- The parent and all 18 children are open with ready-for-agent and enhancement labels.
- Every child links the real parent and exactly its concrete blockers; issue ids and dependency order are verified.
- All 415 owning source-clause texts, 46 parent user stories, 36 named test scenarios and 144 child acceptance criteria are present in their actual live bodies.
- The parent was created once and was not edited or closed after child publication. No existing issue was changed.
- The source design SHA-256 is unchanged. Publication establishes tracker coverage, not code delivery, test passage or authorization to operate on the real owner archive.

## Verified body fingerprints

| Issue | Canonical body SHA-256 |
| --- | --- |
| [PRD / #123](https://github.com/liudesheng9/zet/issues/123) | `b7be0694acd18fcd013bcea195ad1250248c652759affd8c931c3c774775609d` |
| [S01 / #124](https://github.com/liudesheng9/zet/issues/124) | `e23b4c09c540f221e386593fd09c74c0bfcb04a59d73046a547d7585a6827330` |
| [S02 / #125](https://github.com/liudesheng9/zet/issues/125) | `177c357cd0d1d4d4b772b204cb3956aef149a4082a444d511c283743646ae22b` |
| [S03 / #126](https://github.com/liudesheng9/zet/issues/126) | `e45bafeb675b23704de219f046170b5681f85146d93a48f1dbb70ee2f1c2be87` |
| [S04 / #127](https://github.com/liudesheng9/zet/issues/127) | `c44dc37c3fa6047e670a4897b781096ac951e3f6a2571738d76cda72a2db444e` |
| [S05 / #128](https://github.com/liudesheng9/zet/issues/128) | `34d937cffcd8fa12730f484a01c4306b78915b930949cd6aaeed6fcddb77f9ef` |
| [S06 / #129](https://github.com/liudesheng9/zet/issues/129) | `b727a0bbf88462da073b6b96bd45760d0beeecc000d7b48719633b92d39fdd44` |
| [S07 / #130](https://github.com/liudesheng9/zet/issues/130) | `910dba9d2d75f53dceac83869c8937d424cbbb25f134cd6cfcea0438ddba82d1` |
| [S08 / #131](https://github.com/liudesheng9/zet/issues/131) | `a3c68a283af53d4eb74538a4199036bf8f067fb97d6b4a656e9d7bc86bcef05e` |
| [S09 / #132](https://github.com/liudesheng9/zet/issues/132) | `f7c7076208b6d4a0f2cfbcaaea9b75f65561be9bae89bc89b3ee7312533b4be5` |
| [S10 / #133](https://github.com/liudesheng9/zet/issues/133) | `5fb64ae5799b3dd452d8dfc37b53b334645d86d476e44fd22ca28265897b0ea3` |
| [S11 / #134](https://github.com/liudesheng9/zet/issues/134) | `bc00ef5947471119b4be8952a539ef79a4b6a1c050449019fdd6d2c599f89ab3` |
| [S12 / #135](https://github.com/liudesheng9/zet/issues/135) | `a4c6498ec1e9274b7de417bbea264b4d8b4f617833dfe82b45cd43c63ca0b7b8` |
| [S13 / #136](https://github.com/liudesheng9/zet/issues/136) | `f2e6e2ef4e952c29ac9bd8aceb06a0ac53d9be8db35fc6d47ef4207f756a1e7f` |
| [S14 / #137](https://github.com/liudesheng9/zet/issues/137) | `1f59ab093b492f3bf7f1e9ed2cd37a9d34788e21737e4a7294d8c6dfec8d8186` |
| [S15 / #138](https://github.com/liudesheng9/zet/issues/138) | `6f92a5b890b8f8b8e2fa9d38916d2b0dffe725a916309e94aee0706dcefde71d` |
| [S16 / #139](https://github.com/liudesheng9/zet/issues/139) | `17a6b2e2c73cbc02c82bca4c990d6d0fab72d1b6cea8653072eb30c26ff7fb63` |
| [S17 / #140](https://github.com/liudesheng9/zet/issues/140) | `6ed8e4910213612eec895c0dbca2720a9c6b2360c2cad357c4f13d96ee3efcf4` |
| [S18 / #141](https://github.com/liudesheng9/zet/issues/141) | `6414f4dbb12d3d52bd98093da4c20fbb3abf81918cee4afdeec7a5a7b1fd8c5a` |

## Published breakdown

| Slice | Type | Title | Blocked by | Stories | Acceptance criteria |
| --- | --- | --- | --- | --- | --- |
| [S01 / #124](https://github.com/liudesheng9/zet/issues/124) | AFK | Lock and unlock a whole tree through the GUI with shared write protection | None | US001, US002, US003, US004, US005, US008, US010, US011, US012, US013, US043, US046 | 15 |
| [S02 / #125](https://github.com/liudesheng9/zet/issues/125) | AFK | Keep locked Cards readable and linkable while preserving external pins | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) | US006, US007, US008, US009, US010 | 8 |
| [S03 / #126](https://github.com/liudesheng9/zet/issues/126) | AFK | Expose consistent Lock state through every read and normal GUI refresh | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) | US018, US019, US043 | 8 |
| [S04 / #127](https://github.com/liudesheng9/zet/issues/127) | AFK | Edit Lock notes and inspect retained owner history | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) | US014 | 7 |
| [S05 / #128](https://github.com/liudesheng9/zet/issues/128) | AFK | Review GUI Lock plans and preserve drafts when restrictions change | [S01 / #124](https://github.com/liudesheng9/zet/issues/124), [S03 / #126](https://github.com/liudesheng9/zet/issues/126), [S04 / #127](https://github.com/liudesheng9/zet/issues/127) | US011, US012, US016, US017, US019 | 8 |
| [S06 / #129](https://github.com/liudesheng9/zet/issues/129) | AFK | Detect exact integrity faults and keep owner recovery reachable | [S01 / #124](https://github.com/liudesheng9/zet/issues/124), [S04 / #127](https://github.com/liudesheng9/zet/issues/127) | US015, US021, US035, US036 | 10 |
| [S07 / #130](https://github.com/liudesheng9/zet/issues/130) | AFK | Install lock protection safely and preserve it across Card migrations | [S01 / #124](https://github.com/liudesheng9/zet/issues/124), [S06 / #129](https://github.com/liudesheng9/zet/issues/129) | US033, US034, US036, US037, US043 | 9 |
| [S08 / #131](https://github.com/liudesheng9/zet/issues/131) | AFK | Show Lock marks and reliable integrity diagnostics in Session and status | [S01 / #124](https://github.com/liudesheng9/zet/issues/124), [S03 / #126](https://github.com/liudesheng9/zet/issues/126), [S06 / #129](https://github.com/liudesheng9/zet/issues/129) | US020, US021 | 6 |
| [S09 / #132](https://github.com/liudesheng9/zet/issues/132) | AFK | Preserve structured MCP refusals and safe identified receipt replay | [S01 / #124](https://github.com/liudesheng9/zet/issues/124), [S03 / #126](https://github.com/liudesheng9/zet/issues/126) | US022, US023, US024 | 7 |
| [S10 / #133](https://github.com/liudesheng9/zet/issues/133) | AFK | Aggregate Lock refusals in rollback-only batch previews | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) | US025 | 7 |
| [S11 / #134](https://github.com/liudesheng9/zet/issues/134) | AFK | Empty a Topic or Literature tree while keeping its root | [S01 / #124](https://github.com/liudesheng9/zet/issues/124), [S02 / #125](https://github.com/liudesheng9/zet/issues/125) | US026, US027, US028, US029, US031 | 10 |
| [S12 / #135](https://github.com/liudesheng9/zet/issues/135) | AFK | Unify archive deletion under zt del with Session and Lock exclusion | [S11 / #134](https://github.com/liudesheng9/zet/issues/134), [S08 / #131](https://github.com/liudesheng9/zet/issues/131) | US028, US029, US030, US031 | 9 |
| [S13 / #136](https://github.com/liudesheng9/zet/issues/136) | AFK | Include portable Lock history and integrity evidence in every dump | [S01 / #124](https://github.com/liudesheng9/zet/issues/124), [S04 / #127](https://github.com/liudesheng9/zet/issues/127), [S06 / #129](https://github.com/liudesheng9/zet/issues/129) | US032 | 6 |
| [S14 / #137](https://github.com/liudesheng9/zet/issues/137) | AFK | Stop digest write modes before dispatch to locked destinations | [S03 / #126](https://github.com/liudesheng9/zet/issues/126), [S09 / #132](https://github.com/liudesheng9/zet/issues/132) | US038, US041 | 6 |
| [S15 / #138](https://github.com/liudesheng9/zet/issues/138) | AFK | Read locked books in idea-grow and file only into unlocked Topics | [S03 / #126](https://github.com/liudesheng9/zet/issues/126), [S09 / #132](https://github.com/liudesheng9/zet/issues/132) | US039, US041 | 6 |
| [S16 / #139](https://github.com/liudesheng9/zet/issues/139) | AFK | Keep hub, register and organize plans within Lock and pin limits | [S02 / #125](https://github.com/liudesheng9/zet/issues/125), [S03 / #126](https://github.com/liudesheng9/zet/issues/126), [S09 / #132](https://github.com/liudesheng9/zet/issues/132), [S10 / #133](https://github.com/liudesheng9/zet/issues/133) | US040, US041 | 6 |
| [S17 / #140](https://github.com/liudesheng9/zet/issues/140) | AFK | Publish lock-aware frozen Workflows and agent integrations | [S14 / #137](https://github.com/liudesheng9/zet/issues/137), [S15 / #138](https://github.com/liudesheng9/zet/issues/138), [S16 / #139](https://github.com/liudesheng9/zet/issues/139) | US022, US041, US042, US046 | 7 |
| [S18 / #141](https://github.com/liudesheng9/zet/issues/141) | AFK | Qualify Tree Lock release behavior and synchronize domain documentation | [S02 / #125](https://github.com/liudesheng9/zet/issues/125), [S05 / #128](https://github.com/liudesheng9/zet/issues/128), [S07 / #130](https://github.com/liudesheng9/zet/issues/130), [S08 / #131](https://github.com/liudesheng9/zet/issues/131), [S10 / #133](https://github.com/liudesheng9/zet/issues/133), [S12 / #135](https://github.com/liudesheng9/zet/issues/135), [S13 / #136](https://github.com/liudesheng9/zet/issues/136), [S17 / #140](https://github.com/liudesheng9/zet/issues/140) | US043, US044, US045, US046 | 9 |



## Approved test boundaries

Use the existing real GUI/API/browser, stdio MCP, shell/pipe and Session line/PTY paths; SQLite fixtures prove trigger/backstop, migration and full rollback; dump tests extract actual archives; digest/grow/hub tests use existing fake-job and frozen-install harnesses. Release qualification uses isolated archives only. No new mock policy framework, production digest or owner-tree mutation is required.

## Coverage summary

415 source clauses, 46 user stories, 36 named acceptance scenarios and 144 child acceptance criteria have explicit owners. Code/SQL/diagram examples are converted to their behavioral/schema contracts in issue prose; repository file paths are replaced with functional module names.

## Named acceptance-scenario owners

| Scenario | Owning slice |
| --- | --- |
| T01 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T02 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T03 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T04 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T05 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| T06 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T07 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| T08 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| T09 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| T10 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T11 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T12 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T13 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| T14 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| T15 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| T16 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| T17 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| T18 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| T19 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| T20 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| T21 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| T22 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| T23 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| T24 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| D01 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| D02 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| D03 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| D04 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| D05 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| D06 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| D07 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| D08 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| D09 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| D10 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| D11 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| D12 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |



## Source-clause coverage

| Clause | Source section / line | Owning slice |
| --- | --- | --- |
| R001 | §0, line 3 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R002 | §0, line 5 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R003 | §1, line 13 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R004 | §1, line 18 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R005 | §1, line 32 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R006 | §1, line 38 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R007 | §1, line 39 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R008 | §1, line 40 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R009 | §1, line 41 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R010 | §1, line 42 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R011 | §1, line 43 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R012 | §1, line 44 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R013 | §1, line 45 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R014 | §1, line 46 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R015 | §1.1, line 50 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R016 | §1.1, line 52 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R017 | §1.1, line 53 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R018 | §1.1, line 54 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R019 | §1.1, line 55 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R020 | §1.1, line 57 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R021 | §1.1, line 62 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R022 | §1.2, line 67 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R023 | §1.2, line 71 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R024 | §1.2, line 72 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R025 | §1.2, line 73 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R026 | §1.2, line 74 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R027 | §1.2, line 75 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R028 | §1.2, line 76 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R029 | §1.2, line 77 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R030 | §1.2, line 78 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R031 | §2, line 84 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R032 | §2, line 85 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R033 | §2, line 86 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R034 | §2, line 87 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R035 | §2, line 88 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R036 | §2, line 89 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R037 | §2, line 90 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R038 | §2, line 91 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R039 | §2, line 92 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R040 | §2, line 93 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R041 | §2, line 94 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R042 | §2, line 95 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R043 | §2, line 96 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R044 | §2, line 97 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R045 | §2, line 98 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R046 | §2, line 99 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R047 | §2, line 100 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R048 | §2, line 102 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R049 | §3, line 108 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R050 | §3, line 113 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R051 | §3, line 114 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R052 | §3, line 115 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R053 | §3, line 116 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R054 | §3, line 117 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R055 | §3, line 118 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R056 | §3, line 119 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R057 | §3, line 120 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R058 | §3, line 121 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R059 | §3, line 122 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R060 | §3, line 123 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R061 | §3, line 124 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R062 | §3, line 125 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R063 | §3, line 126 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R064 | §3, line 127 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R065 | §3, line 128 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R066 | §3, line 129 | [S15 / #138](https://github.com/liudesheng9/zet/issues/138) |
| R067 | §3, line 131 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R068 | §4.1, line 139 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R069 | §4.1, line 144 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R070 | §4.1, line 148 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R071 | §4.1, line 152 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R072 | §4.1, line 154 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R073 | §4.1, line 155 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R074 | §4.1, line 156 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R075 | §4.1, line 157 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R076 | §4.1, line 158 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R077 | §4.1, line 159 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R078 | §4.1, line 161 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R079 | §4.2, line 166 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R080 | §4.2, line 172 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R081 | §4.2, line 175 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R082 | §4.2, line 179 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R083 | §4.3, line 185 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R084 | §4.3, line 190 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R085 | §4.3, line 195 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R086 | §4.3, line 199 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R087 | §4.3, line 205 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R088 | §4.4, line 211 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R089 | §4.4, line 214 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R090 | §4.4, line 218 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R091 | §4.4, line 223 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R092 | §5.1, line 230 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R093 | §5.1, line 233 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R094 | §5.1, line 278 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R095 | §5.1, line 284 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R096 | §5.1, line 288 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R097 | §5.1, line 292 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R098 | §5.2, line 298 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R099 | §5.2, line 303 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R100 | §5.2, line 304 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R101 | §5.2, line 305 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R102 | §5.2, line 306 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R103 | §5.2, line 307 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R104 | §5.2, line 309 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R105 | §5.2, line 313 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R106 | §5.3, line 319 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R107 | §5.3, line 322 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R108 | §5.3, line 328 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R109 | §5.3, line 333 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R110 | §5.3, line 339 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R111 | §5.4, line 346 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R112 | §5.4, line 351 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R113 | §5.4, line 409 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R114 | §5.4, line 413 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R115 | §5.4, line 419 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R116 | §6.1, line 427 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R117 | §6.1, line 432 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R118 | §6.1, line 433 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R119 | §6.1, line 434 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R120 | §6.1, line 435 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R121 | §6.1, line 436 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R122 | §6.1, line 437 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R123 | §6.1, line 438 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R124 | §6.1, line 439 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R125 | §6.1, line 440 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R126 | §6.1, line 441 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R127 | §6.1, line 442 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R128 | §6.1, line 443 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R129 | §6.1, line 445 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R130 | §6.1, line 447 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R131 | §6.1, line 457 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R132 | §6.1, line 462 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R133 | §6.2, line 467 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R134 | §6.2, line 471 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R135 | §6.2, line 473 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R136 | §6.2, line 474 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R137 | §6.2, line 475 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R138 | §6.2, line 477 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R139 | §6.2, line 478 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R140 | §6.2, line 480 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R141 | §6.2, line 487 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R142 | §6.3, line 492 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R143 | §6.3, line 494 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R144 | §6.3, line 495 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R145 | §6.3, line 497 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R146 | §6.3, line 498 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R147 | §6.3, line 499 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R148 | §6.3, line 500 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R149 | §6.3, line 501 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R150 | §6.3, line 503 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R151 | §6.4, line 510 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R152 | §6.4, line 516 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R153 | §6.4, line 520 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R154 | §7.1, line 528 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R155 | §7.1, line 530 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R156 | §7.1, line 542 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R157 | §7.1, line 547 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R158 | §7.1, line 550 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R159 | §7.2, line 557 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R160 | §7.2, line 558 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R161 | §7.2, line 559 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R162 | §7.2, line 560 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R163 | §7.2, line 561 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R164 | §7.2, line 562 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R165 | §7.2, line 563 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R166 | §7.2, line 564 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R167 | §7.2, line 565 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R168 | §7.2, line 566 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R169 | §7.2, line 567 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R170 | §7.2, line 568 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R171 | §7.2, line 569 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R172 | §7.2, line 571 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R173 | §7.3, line 578 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R174 | §7.3, line 583 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R175 | §7.3, line 585 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R176 | §7.3, line 586 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R177 | §7.3, line 587 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R178 | §7.3, line 588 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R179 | §7.3, line 590 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R180 | §7.4, line 597 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R181 | §7.4, line 601 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R182 | §8.1, line 609 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R183 | §8.1, line 611 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R184 | §8.1, line 622 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R185 | §8.1, line 627 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R186 | §8.1, line 632 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R187 | §8.2, line 637 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R188 | §8.2, line 642 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R189 | §8.2, line 647 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R190 | §8.3, line 652 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R191 | §8.3, line 655 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R192 | §8.3, line 656 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R193 | §8.3, line 657 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R194 | §8.3, line 658 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R195 | §8.3, line 660 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R196 | §8.3, line 662 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R197 | §8.3, line 664 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R198 | §8.3, line 666 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R199 | §8.3, line 669 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R200 | §8.3, line 676 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R201 | §9.1, line 684 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R202 | §9.1, line 686 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R203 | §9.1, line 693 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R204 | §9.1, line 697 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R205 | §9.2, line 703 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R206 | §9.2, line 707 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R207 | §9.2, line 711 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R208 | §9.2, line 715 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R209 | §9.3, line 723 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R210 | §9.3, line 727 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R211 | §9.3, line 733 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R212 | §10.1, line 741 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R213 | §10.1, line 746 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R214 | §10.1, line 747 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R215 | §10.1, line 748 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R216 | §10.1, line 749 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R217 | §10.1, line 751 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R218 | §10.1, line 756 | [S04 / #127](https://github.com/liudesheng9/zet/issues/127) |
| R219 | §10.2, line 764 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R220 | §10.2, line 768 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R221 | §10.2, line 770 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R222 | §10.2, line 771 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R223 | §10.2, line 773 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R224 | §10.2, line 775 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R225 | §10.2, line 777 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R226 | §10.2, line 781 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R227 | §10.2, line 785 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R228 | §10.3, line 792 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R229 | §10.3, line 796 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R230 | §10.3, line 801 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R231 | §11, line 807 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R232 | §11, line 811 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R233 | §11, line 816 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R234 | §11, line 818 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R235 | §11, line 822 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R236 | §11, line 827 | [S08 / #131](https://github.com/liudesheng9/zet/issues/131) |
| R237 | §12.1, line 836 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R238 | §12.1, line 843 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R239 | §12.1, line 848 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R240 | §12.1, line 852 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R241 | §12.1, line 853 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R242 | §12.1, line 854 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R243 | §12.1, line 855 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R244 | §12.1, line 857 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R245 | §12.1, line 861 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R246 | §12.2, line 868 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R247 | §12.2, line 870 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R248 | §12.2, line 879 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R249 | §12.2, line 885 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R250 | §12.2, line 889 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R251 | §12.3, line 895 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R252 | §12.3, line 900 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R253 | §12.3, line 902 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R254 | §12.3, line 903 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R255 | §12.3, line 904 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R256 | §12.3, line 906 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R257 | §12.3, line 907 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R258 | §12.3, line 909 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R259 | §12.3, line 914 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R260 | §12.3, line 917 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R261 | §12.3, line 919 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R262 | §12.3, line 923 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R263 | §12.3, line 927 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R264 | §12.4, line 934 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R265 | §12.4, line 936 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R266 | §12.4, line 937 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R267 | §12.4, line 938 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R268 | §12.4, line 939 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R269 | §12.4, line 941 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R270 | §12.4, line 946 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R271 | §12.4, line 953 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R272 | §12.4, line 959 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R273 | §12.4, line 964 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R274 | §12.4, line 966 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R275 | §12.4, line 970 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R276 | §12.5, line 976 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R277 | §12.5, line 981 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R278 | §13.1, line 989 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R279 | §13.1, line 992 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R280 | §13.1, line 994 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R281 | §13.1, line 995 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R282 | §13.1, line 996 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R283 | §13.1, line 997 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R284 | §13.1, line 998 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R285 | §13.1, line 999 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R286 | §13.1, line 1001 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R287 | §13.1, line 1003 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R288 | §13.1, line 1006 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R289 | §13.1, line 1010 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R290 | §13.2, line 1017 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R291 | §13.2, line 1018 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R292 | §13.2, line 1019 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R293 | §13.2, line 1020 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R294 | §13.2, line 1022 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R295 | §13.2, line 1025 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R296 | §13.3, line 1033 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R297 | §13.3, line 1038 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R298 | §13.3, line 1044 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R299 | §13.4, line 1050 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R300 | §13.4, line 1054 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R301 | §14.1, line 1062 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R302 | §14.1, line 1066 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R303 | §14.1, line 1071 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R304 | §14.2, line 1078 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R305 | §14.2, line 1083 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R306 | §14.2, line 1085 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R307 | §14.2, line 1086 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R308 | §14.2, line 1087 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R309 | §14.2, line 1088 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R310 | §14.2, line 1091 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R311 | §14.2, line 1093 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R312 | §14.2, line 1096 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R313 | §14.2, line 1098 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R314 | §14.3, line 1105 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R315 | §14.3, line 1109 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R316 | §14.3, line 1114 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R317 | §14.3, line 1116 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R318 | §14.3, line 1117 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R319 | §14.3, line 1118 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R320 | §14.3, line 1119 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R321 | §14.3, line 1120 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R322 | §14.3, line 1121 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R323 | §14.3, line 1123 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R324 | §14.3, line 1129 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R325 | §15.1, line 1136 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R326 | §15.1, line 1141 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R327 | §15.2, line 1149 | [S14 / #137](https://github.com/liudesheng9/zet/issues/137) |
| R328 | §15.2, line 1150 | [S14 / #137](https://github.com/liudesheng9/zet/issues/137) |
| R329 | §15.2, line 1151 | [S14 / #137](https://github.com/liudesheng9/zet/issues/137) |
| R330 | §15.2, line 1152 | [S15 / #138](https://github.com/liudesheng9/zet/issues/138) |
| R331 | §15.2, line 1153 | [S16 / #139](https://github.com/liudesheng9/zet/issues/139) |
| R332 | §15.2, line 1154 | [S16 / #139](https://github.com/liudesheng9/zet/issues/139) |
| R333 | §15.2, line 1156 | [S14 / #137](https://github.com/liudesheng9/zet/issues/137) |
| R334 | §15.2, line 1158 | [S14 / #137](https://github.com/liudesheng9/zet/issues/137) |
| R335 | §15.2, line 1162 | [S15 / #138](https://github.com/liudesheng9/zet/issues/138) |
| R336 | §15.2, line 1168 | [S16 / #139](https://github.com/liudesheng9/zet/issues/139) |
| R337 | §15.3, line 1173 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R338 | §15.3, line 1178 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R339 | §15.3, line 1183 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R340 | §15.3, line 1189 | [S17 / #140](https://github.com/liudesheng9/zet/issues/140) |
| R341 | §16, line 1195 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R342 | §16, line 1199 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R343 | §16, line 1205 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R344 | §16, line 1209 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R345 | §17, line 1216 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R346 | §17, line 1221 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R347 | §17, line 1222 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R348 | §17, line 1223 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R349 | §17, line 1224 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R350 | §17, line 1225 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R351 | §17, line 1226 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R352 | §17, line 1228 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R353 | §17, line 1232 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R354 | §18.1, line 1239 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R355 | §18.1, line 1244 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R356 | §18.1, line 1245 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R357 | §18.1, line 1246 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R358 | §18.1, line 1247 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R359 | §18.1, line 1248 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R360 | §18.1, line 1249 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R361 | §18.1, line 1250 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R362 | §18.1, line 1251 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R363 | §18.1, line 1252 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R364 | §18.1, line 1253 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R365 | §18.1, line 1254 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R366 | §18.1, line 1255 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R367 | §18.1, line 1256 | [S05 / #128](https://github.com/liudesheng9/zet/issues/128) |
| R368 | §18.1, line 1257 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R369 | §18.1, line 1258 | [S09 / #132](https://github.com/liudesheng9/zet/issues/132) |
| R370 | §18.1, line 1259 | [S10 / #133](https://github.com/liudesheng9/zet/issues/133) |
| R371 | §18.1, line 1260 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R372 | §18.1, line 1261 | [S03 / #126](https://github.com/liudesheng9/zet/issues/126) |
| R373 | §18.1, line 1262 | [S02 / #125](https://github.com/liudesheng9/zet/issues/125) |
| R374 | §18.1, line 1263 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R375 | §18.1, line 1264 | [S07 / #130](https://github.com/liudesheng9/zet/issues/130) |
| R376 | §18.1, line 1265 | [S01 / #124](https://github.com/liudesheng9/zet/issues/124) |
| R377 | §18.1, line 1266 | [S06 / #129](https://github.com/liudesheng9/zet/issues/129) |
| R378 | §18.1, line 1267 | [S13 / #136](https://github.com/liudesheng9/zet/issues/136) |
| R379 | §18.1, line 1269 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R380 | §18.2, line 1278 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R381 | §18.2, line 1279 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R382 | §18.2, line 1280 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R383 | §18.2, line 1281 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R384 | §18.2, line 1282 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R385 | §18.2, line 1283 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R386 | §18.2, line 1284 | [S11 / #134](https://github.com/liudesheng9/zet/issues/134) |
| R387 | §18.2, line 1285 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R388 | §18.2, line 1286 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R389 | §18.2, line 1287 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R390 | §18.2, line 1288 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R391 | §18.2, line 1289 | [S12 / #135](https://github.com/liudesheng9/zet/issues/135) |
| R392 | §18.3, line 1293 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R393 | §18.3, line 1297 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R394 | §18.3, line 1299 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R395 | §18.3, line 1301 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R396 | §18.3, line 1303 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R397 | §18.3, line 1305 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R398 | §18.3, line 1306 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R399 | §18.3, line 1309 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R400 | §18.3, line 1312 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R401 | §18.3, line 1319 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R402 | §18.3, line 1325 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R403 | §19, line 1330 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R404 | §19, line 1335 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R405 | §19, line 1336 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R406 | §19, line 1337 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R407 | §19, line 1338 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R408 | §19, line 1339 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R409 | §19, line 1340 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R410 | §19, line 1341 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R411 | §19, line 1342 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R412 | §19, line 1343 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R413 | §19, line 1344 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R414 | §19, line 1346 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |
| R415 | §19, line 1352 | [S18 / #141](https://github.com/liudesheng9/zet/issues/141) |



## Published PRD body

## Problem Statement

Reviewed agent and human trees cannot currently be frozen. Every permitted writer can still rewrite, extend, renumber or remove finished work, and outside movement/deletion can invalidate authored Links. Deletion is split between overlapping commands and cannot retain a tree root for a clean rerun. The owner needs one reliable promise across GUI, Session, shell, MCP and compiler paths.

## Solution

Let the owner review and confirm a whole-tree Lock in the GUI. All clients keep reading and linking its Cards while authored identity, membership, addresses, existence and existing outward references remain stable until an explicit owner unlock. Show current state and faults, retain history and dump evidence, and join deletion under one command with a root-preserving reset. Keep the existing architecture and the policy-based authority boundary.

## User Stories

1. **US001** As an owner, I want to lock a whole Topic or Literature tree in either Workspace, so that finished work remains fixed.
2. **US002** As an owner, I want to have my own shell, Session and GUI writes obey a Lock, so that my mistakes cannot change reviewed work.
3. **US003** As an owner, I want to prevent every authored edit and metadata change in a Locked tree, so that its authored identity stays trustworthy.
4. **US004** As an owner, I want to prevent new Cards and incoming moves into a Locked tree, so that the reviewed membership remains exact.
5. **US005** As an owner, I want to prevent moves, inserts, Citation-key renames, Side compaction and deletes that disturb locked members, so that their addresses and existence stay fixed.
6. **US006** As a reader, I want to read, search, navigate, export and reference locked Cards normally, so that reviewed work remains useful.
7. **US007** As an author, I want to add or remove incoming Links and write Replies to locked Cards, so that the conversation can continue without editing the source.
8. **US008** As an owner, I want to keep outgoing Links from locked Cards resolvable, so that outside changes cannot break my reviewed references.
9. **US009** As an author, I want to edit an unlocked Pinned target without moving it, so that reference stability does not freeze unrelated meaning.
10. **US010** As an owner, I want to see preexisting Broken links before locking, so that I understand which references are outside the baseline promise.
11. **US011** As an owner, I want to review the exact members, pins, note and seal before confirming, so that I know what I am freezing.
12. **US012** As an owner, I want to reject stale confirmations after another write or relock, so that an old dialog cannot approve a different promise.
13. **US013** As an owner, I want to unlock exactly one active Lock through the GUI, so that I can deliberately revise a finished tree.
14. **US014** As an owner, I want to edit a Lock note and inspect retained history, so that I can understand why and when the promise changed.
15. **US015** As an owner, I want to recover a faulted or orphaned Lock through its retained record, so that a missing root cannot make owner recovery unreachable.
16. **US016** As a GUI user, I want to recognize bands, seals, pins and disabled write actions with text explanations, so that the current restrictions are clear.
17. **US017** As a GUI author, I want to retain unsaved drafts when a destination becomes locked, so that a race does not discard my writing.
18. **US018** As a reader, I want to see consistent lock and pin annotations on every read projection, so that lean and full readers agree about allowed work.
19. **US019** As a GUI user, I want to notice lock-only changes through normal refresh, so that an open page does not keep stale editing controls.
20. **US020** As a Session user, I want to see locked and pinned marks and an unlock hint, so that terminal work obeys the same promise.
21. **US021** As an operator, I want to inspect counts and full Lock integrity through status, so that I can identify faults without treating read errors as healthy zeroes.
22. **US022** As an agent, I want to receive a structured Lock refusal through every write tool, so that I can revise my plan without parsing English or trying a bypass.
23. **US023** As an agent, I want to have an unseen failed batch roll back all its writes and receipts, so that a Lock refusal leaves no partial batch.
24. **US024** As an agent, I want to replay an identical historical receipt without rewriting a locked Card, so that response recovery stays safe and idempotent.
25. **US025** As an agent planner, I want to preview all independently resolvable locked items and identify blocked references, so that I can revise a complete plan honestly.
26. **US026** As an owner, I want to empty a tree while retaining its root and inbound root Links, so that a digest can start again at the first Regular Card.
27. **US027** As a Session user, I want to empty the current tree through del keep-root, so that I can reset its Regular Cards while preserving my navigation root.
28. **US028** As an owner, I want to use one delete command for a subtree, whole tree, retained root or entire archive, so that deletion has one predictable interface.
29. **US029** As an owner, I want to require the correct confirmation word and validate arguments before prompting, so that accidental or malformed deletions do nothing.
30. **US030** As an owner, I want to refuse all-archive deletion while any Lock or Session is active, so that protected work and live navigation cannot be swept away.
31. **US031** As an owner, I want to preserve receipts, files and the specified Spread behavior during deletion, so that consolidation does not become unrelated cleanup.
32. **US032** As an owner, I want to capture active and historical Locks in every dump format, so that backups retain inspectable promises and fault evidence.
33. **US033** As an operator, I want to install the schema additively with a consistent backup and retryable failure, so that upgrading does not damage existing Cards.
34. **US034** As an operator, I want to preserve row identities and trigger protection during future Card rebuilds, so that migration does not quietly remove the promise.
35. **US035** As an owner, I want to see exact integrity faults which remain visible until reviewed recovery, so that repair does not silently certify changed content.
36. **US036** As an operator, I want to detect missing or altered triggers before startup repair, so that a repaired backstop does not erase evidence of lost protection.
37. **US037** As a maintainer, I want to require explicit reseal evidence for an intentional authored-representation migration, so that a migration cannot disguise an ordinary edit.
38. **US038** As a digest operator, I want to stop known locked write destinations before dispatching jobs, so that a rerun does not waste work or overwrite reviewed trees.
39. **US039** As an idea-grow operator, I want to read a locked source book and file only into unlocked agent Topics, so that finished source work can generate new thought safely.
40. **US040** As a hub or organize operator, I want to read locked members while planning only permitted edits and structural changes, so that navigation maintenance cannot move pins or locked Cards.
41. **US041** As a workflow operator, I want to preserve staged drafts and committed receipts when a tree locks mid-run, so that a stopped stage reports its actual partial progress.
42. **US042** As an agent operator, I want to use newly frozen Workflows and updated skills which preserve structured refusals, so that the normal agent path respects owner policy without GUI control.
43. **US043** As a maintainer, I want to reuse existing storage, Link index, revision and test seams, so that the implementation remains small and auditable.
44. **US044** As a maintainer, I want to verify public behavior and real rollback in isolated archives, so that tests prove the promise without touching the owner archive.
45. **US045** As a maintainer, I want to keep domain language, help and block documentation synchronized with code, so that future writers and clients understand the same contract.
46. **US046** As an owner, I want to retain explicit limits on policy authority and external documents, so that I do not mistake a local content promise for adversarial security or source-document preservation.

## Implementation Decisions

1. D1–D7 are fixed: only whole real trees, no new members, pinned existing external targets cannot move/delete, GUI-only owner control, owner also bound, no delete-all with active Locks, and both Workspaces supported.

2. One tree_lock module and four additive SQLite tables retain current/history Locks, member baselines, pin pairs and events. Reuse existing Cards, Access, transactions, writer mutex, Link index, revision, Snapshot/scoped reads, delete planning and frozen installs.

3. Authored identity reuses the existing parsed title/body/BibTeX/type-field fingerprint exactly, with address/row/Workspace/kind checked separately. A deterministic address-sorted SHA-256 range seal excludes generated Reverse links and does not freeze external source documents or target meaning.

4. Every storage writer, including owner shell/Session, metadata setters, placement, batch/paragraph edits, Reply/hub destinations and complete structural mappings/deletes, checks the same active membership/pin facts. Six SQL triggers are a backstop, never a replacement for useful pre-prompt/application refusals.

5. Use deterministic SQL fingerprint/final-tree-root wrappers on every writable connection. Final-root lookup catches insertion/move-in regardless of temporary parent order. No connection-opening migration recursion or public trigger-disable control is introduced.

6. Cross-tree authored Links which resolve at lock time create deduplicated source/target pins. Pins are non-transitive, combine across Locks and permit target text/metadata edits without address/existence changes. Originally broken addresses are warned and unpinned for that baseline until relock.

7. GUI Lock/unlock/note controls plan and confirm through the existing API token, with stateless canonical plan_hash binding action, archive identity/revision, root/Lock id, requested note, baseline/pins/warnings. Recompute under the writer mutex; release it while a dialog is open. No agent tool or terminal Lock command exists.

8. Keep note bounds, UTC timestamps, complete retained initial snapshots/history and atomic events; changed note/reseal/fault/Lock state advances the shared revision once. Postcommit log failure cannot roll back a committed promise. Active orphaned/faulted Locks stay recoverable by retained id.

9. Full and lean read projections get lock/pinned_by and truthful client writable state; tree lists get locked. Bulk annotations share the same read view and revision, and the existing browser fingerprint observes metadata-only changes without a new cache or polling loop.

10. Typed card_locked refusals preserve existing transport envelopes and reason vocabulary. Real batch writes remain bounded, fail-fast and atomic; historical identical receipts replay without mutation. Preview savepoints aggregate independent Lock failures and explicitly label dependent items blocked, then roll back all simulation.

11. Consolidate subtree, whole-tree, keep-root and all-archive deletion under zt del through one existing-plan transaction path. Retain exact selector/kind/confirmation rules, shell-only all, one-release non-destructive clear notice and later notice removal.

12. Keep-root preserves root authored identity/BibTeX and inbound root Links, removes Regular Cards/Spread positions, reports broken child Links, retains allocation/receipts and permits the next /1. All resets Topic allocation but preserves old-clear metadata/file/receipt/history behavior and excludes every active Lock and Session across confirmation/commit.

13. Serialize all-archive Session exclusion with the existing registration mutex in edit-then-registration order. Do not add a new global mutex or recurse into a held registration lock.

14. Integrity scans compare exact root/member/pin/index/seal/catalog facts and retain deduplicated active-Lock fault latches. Detect lost/altered triggers before startup repair; no automatic reseal, fabricated baseline or writable startup with a missing backstop.

15. Upgrade additively with a consistent SQLite backup, supported version validation and retryable transaction failure. Future Card rebuilds preserve row identities/evidence and canonical triggers; intentional authored-representation changes require a specifically designed trusted transaction and audited old/new reseal. No reseal migration ships now.

16. Dumps include deterministic versioned locks.json from the same Card/identity/revision view, active and historical evidence, events and integrity facts, through the existing explicit compression entries and cleanup. Restore is not added.

17. Digest, idea-grow, hub/register and organize preparation distinguishes locked sources from actual destinations. Known locked write targets stop before jobs; mid-run Lock refusal preserves staging/prior receipts. Publish new verified frozen Workflow versions and skills, never patch installed frozen code or retry Lock refusals as provider errors.

18. Keep storage proportional to members/pins/events with no complete body copies, indexed ordinary checks, selected bulk read facts and full integrity only at named check points. Synchronize affected block docs and domain/help contracts with code and qualify only isolated archives.

## Testing Decisions

- Test public behavior through the real token-guarded GUI API/browser, real stdio MCP, shell/stdin and line/PTY Session fixtures, dump extraction, startup/migration and existing fake-job/frozen-Workflow harnesses. Prefer these existing highest practical seams over new mock layers.
- Use direct SQLite only as an isolated backstop/tamper/rollback oracle, not as a production alternate client. Compare complete relevant state, fingerprints, pins, events, receipts, allocation and revision; messages alone cannot prove rollback.
- Cover both tree kinds, both Workspaces and Owner/Agent combinations as existing Access permits. Verify all T01–T24 and D01–D12 scenarios from the source design and actual metadata-only refresh, stale confirmation, multi-Lock pins, originally broken references, fault latching and lost-response behavior.
- Keep all fixtures under temporary archive/config roots and GUI ports. No real owner tree, book digest or provider run is required.
- Run focused suites during implementation, then root all-target tests, standalone checker/digest-tool tests, the digest PowerShell harness, affected GUI JavaScript/Workflow assembly and verification, wiki checks and isolated installed-release health/rollback qualification.
- The source design's in-memory SQL-example smoke check validates only its examples with stand-in functions. It is not evidence of production implementation or release tests.

## Out of Scope

Partial-tree/per-Card or timed Locks; automatic locking after a digest; agent/terminal Lock control; secret-based owner authentication; adversarial direct-SQL/token resistance; force deletion; transitive reference freezing; external source-document/artifact availability or pinned target meaning; new permission/dependency/service frameworks; restore/import; generic Lock repair or public reseal; new remote keep-root/delete-all arguments; unrelated Spread cleanup, artifact garbage collection or receipt reset; real owner Lock/migration actions during qualification.

## Further Notes

Source authority: Tree Lock Implementation Design, 2026-10-21, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a; source decisions D1–D7 and all detailed contracts remain in scope. Current repository baseline bc4574e plus the existing unsettled idea-grow working changes must be rechecked at implementation time. The user explicitly approved these 18 focused AFK issues, their dependencies and testing boundaries before publication. Target release is 0.22.0. Publication does not establish implementation.

The six design delivery groups are retained as coverage groups and refined into 18 AFK end-to-end issues. All decisions needed for implementation are specified; real owner selection of a production tree remains outside automated release work. Each child will link this parent and its concrete blockers; this parent is not edited or closed by child publication.

- S01: Lock and unlock a whole tree through the GUI with shared write protection; blockers: none; stories: US001, US002, US003, US004, US005, US008, US010, US011, US012, US013, US043, US046
- S02: Keep locked Cards readable and linkable while preserving external pins; blockers: S01; stories: US006, US007, US008, US009, US010
- S03: Expose consistent Lock state through every read and normal GUI refresh; blockers: S01; stories: US018, US019, US043
- S04: Edit Lock notes and inspect retained owner history; blockers: S01; stories: US014
- S05: Review GUI Lock plans and preserve drafts when restrictions change; blockers: S01, S03, S04; stories: US011, US012, US016, US017, US019
- S06: Detect exact integrity faults and keep owner recovery reachable; blockers: S01, S04; stories: US015, US021, US035, US036
- S07: Install lock protection safely and preserve it across Card migrations; blockers: S01, S06; stories: US033, US034, US036, US037, US043
- S08: Show Lock marks and reliable integrity diagnostics in Session and status; blockers: S01, S03, S06; stories: US020, US021
- S09: Preserve structured MCP refusals and safe identified receipt replay; blockers: S01, S03; stories: US022, US023, US024
- S10: Aggregate Lock refusals in rollback-only batch previews; blockers: S09; stories: US025
- S11: Empty a Topic or Literature tree while keeping its root; blockers: S01, S02; stories: US026, US027, US028, US029, US031
- S12: Unify archive deletion under zt del with Session and Lock exclusion; blockers: S11, S08; stories: US028, US029, US030, US031
- S13: Include portable Lock history and integrity evidence in every dump; blockers: S01, S04, S06; stories: US032
- S14: Stop digest write modes before dispatch to locked destinations; blockers: S03, S09; stories: US038, US041
- S15: Read locked books in idea-grow and file only into unlocked Topics; blockers: S03, S09; stories: US039, US041
- S16: Keep hub, register and organize plans within Lock and pin limits; blockers: S02, S03, S09, S10; stories: US040, US041
- S17: Publish lock-aware frozen Workflows and agent integrations; blockers: S14, S15, S16; stories: US022, US041, US042, US046
- S18: Qualify Tree Lock release behavior and synchronize domain documentation; blockers: S02, S05, S07, S08, S10, S12, S13, S17; stories: US043, US044, US045, US046

## S01 / #124 — Lock and unlock a whole tree through the GUI with shared write protection

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Deliver the safe owner action end to end: a GUI confirmation freezes a real whole tree, snapshots its authored identity and external pins, and makes every supported Card writer obey the promise. Unlock releases exactly that active Lock. This is the shared foundation; do not expose lock controls with incomplete storage protection.

**Type:** AFK. **Slice:** S01. **User stories:** US001, US002, US003, US004, US005, US008, US010, US011, US012, US013, US043, US046.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R001 (§0)** Status: implementation design, 2026-10-21. Not implemented. Target release: zt 0.22.0.

- **R002 (§0)** This document turns Tree Lock Proposal into an implementation plan. The owner's decisions D1–D7 remain fixed. It also specifies the joined delete command. The design uses the current repository at `bc4574e` as its source baseline, together with the idea-grow work already present in the working directory. That work is still uncommitted and must be checked again when its integration is implemented.

- **R003 (§1)** Add **one Rust module, `the tree_lock module`**, to the existing binary. Keep the four small tables in the proposal, in the existing `zt.sqlite3`. Put the checks in the transaction-level functions that already write Cards. Extend the existing readers and clients. Extend the existing `DeletePlan` instead of building a second deletion system.

- **R004 (§1)** Architecture: GUI owner Lock actions use tree_lock; all GUI, MCP, shell, Session, batch, Reply and hub-draft Card writers pass existing Access and shared tree checks into the same SQLite archive, whose six triggers provide a backstop; existing read projections supply lock annotations and Workflow preparation consumes them.

- **R005 (§1)** The application check and the SQL triggers serve different needs. The check returns a useful, structured refusal before a prompt or a write. The triggers stop a forgotten write path. They use the same stored facts and the same fingerprint function. They are not separate permission systems.

- **R006 (§1)** `Access::Owner` / `Access::Agent` — Keep Workspace policy. Both must pass the tree check.

- **R007 (§1)** `zt.edit.lock` and caller-owned SQLite transactions — Serialize lock changes with Card changes. Add no second writer mutex.

- **R008 (§1)** `card::authored_fingerprint` — Reuse its exact authored identity for member snapshots and SQL checks.

- **R009 (§1)** `links` occurrence index — Select outgoing Links when setting a Lock. Do not build another Link graph.

- **R010 (§1)** `metadata.archive_revision` — Advance for lock state, note, or recorded integrity changes too.

- **R011 (§1)** `Snapshot`, scoped reads, GUI polling — Attach lock state from the same database view.

- **R012 (§1)** `DeletePlan` — Add keep-root and all-archive selections. Apply every selection through one transaction function.

- **R013 (§1)** GUI dialogs and token checking — Add the lock panel and its two-step actions.

- **R014 (§1)** Workflow preparation and frozen installs — Check intended destinations and publish new Workflow versions.

- **R015 (§1.1)** Each table retains a different fact required by the features:

- **R016 (§1.1)** `locks`: the current or historical Lock, its note, root, time, and seal.

- **R017 (§1.1)** `lock_members`: the exact Card identities and authored fingerprints promised by an active Lock.

- **R018 (§1.1)** `lock_pins`: the existing external targets whose addresses must survive that Lock.

- **R019 (§1.1)** `lock_events`: the owner's history and persistent integrity-fault evidence.

- **R020 (§1.1)** A boolean on the Tree root cannot detect changed or missing members. Deriving pins only from the current Link index would make a damaged or rebuilt derived index determine protection. JSON blobs would make indexed writer checks and SQL triggers harder. A general permission framework, separate lock service, background watcher, new database, and per-Card lock controls add no required behavior.

- **R021 (§1.1)** The new code needs ordinary structs and free functions. It does not need a repository trait, policy registry, event bus, dependency engine, or generic command framework.

- **R022 (§1.2)** These choices resolve details in the proposal's sketches; they do not reopen D1–D7.

- **R023 (§1.2)** Insert and move-in trigger lookup — Compare the destination Tree root directly with active `locks.root_address`, using `zt_tree_root(address)` — Does not depend on a parent already having reached its final address during a multirow move.

- **R024 (§1.2)** Confirmation freshness — Return a stateless `plan_hash`; require it with the confirmation word — Prevents confirming a different Lock or a changed member/pin plan. No stored pending-plan table.

- **R031 (§2)** D1: whole tree, Tree root only — Resolve a real Topic or Literature Card; snapshot root and every member — 4–6

- **R032 (§2)** D2: no additions — Check final tree on insertion and every mapping destination, including a main-line append — 5, 7

- **R033 (§2)** D3: external targets stay addressable — Snapshot cross-tree Links; reject target deletion, movement, rename, and `into` re-pointing — 4, 7

- **R035 (§2)** D5: owner also bound — Storage check has no Owner exemption — 7

- **R037 (§2)** D7: both Workspaces — Owner may lock either; existing agent Workspace restriction remains — 4, 7

- **R040 (§2)** All write paths — Guard low-level writes and operation plans, including batch, Reply, and hub draft — 7–8

- **R048 (§2)** This work adds no restore command, partial-tree Lock, timed expiry, agent lock authority, password, automatic unlock, force-delete option, or automatic locking when a digest finishes. The owner still reviews the result and locks the tree in the GUI.

- **R049 (§3)** Read the relevant wiki blocks before implementation. Current-source facts below come from the named files; the proposed interfaces later in this document are new design.

- **R053 (§3)** `the Workspace/Access module` — Agent Access refuses human writes and human authored Link rewrites; Owner bypasses that Workspace rewrite restriction. — Tree checks must be independent of the Owner shortcut.

- **R054 (§3)** `the crate-root storage and CLI module`, `the placement module` — Shell/Session can bypass `ops module`. Insert has direct SQL. Moves temporarily set Locations to `__moving__<old>`. — Guard storage functions, not just public `ops` wrappers. Check a complete final mapping before temporary renames start.

- **R055 (§3)** `the shared Card-operations module` — GUI/MCP share transaction-level create/edit functions and destructive plans. — Add friendly checks before confirmation and repeat in the transaction writers.

- **R056 (§3)** `the model module`, `the hub module`, `the math module` — Metadata setters execute Card UPDATEs directly. — Put the authored-edit check in each setter, including clearing fields.

- **R058 (§3)** `the derived Link-index module` — Maintains authored Link occurrences and generated Reverse links; owns archive revision helpers. — Reuse its transaction boundary. A generated refresh must pass on locked members.

- **R067 (§3)** The Card-operations wiki contains older pitfall descriptions followed by later entries that make edits atomic. Follow the current transaction writers, rather than rebuilding an already fixed edit path from an older description.

- **R068 (§4.1)** A Lock belongs to one actual Tree root. A Topic root's address is `3/0`; its tree id is `3`. A Literature root's address and tree id are both `Smith2024`. Keep these values distinct in code and output: `lock.tree` in the proposed public annotation is the **root address**, whereas the existing Card `tree` field continues to be the tree id.

- **R069 (§4.1)** Use the existing `is_tree_root`, `tree_id`, `tree_root_address`, and `is_tree_member` rules. Do not use an unescaped SQL `LIKE '<key>/%'` or compare raw prefixes such as `3` and `30`. Citation-key case remains significant for address resolution. Workspace and Zone rules do not change.

- **R070 (§4.1)** At lock time, the set includes the root even if the tree has no Regular Card. Reject a Regular Card as an anchor. Reject a nonexistent root, an already locked root, and inconsistent membership such as a member whose Workspace differs from its root.

- **R071 (§4.1)** While active, a Lock promises:

- **R072 (§4.1)** Every snapshotted row still exists, at its exact address, with the same Workspace and Card kind.

- **R073 (§4.1)** Every member has its snapshotted authored fingerprint.

- **R074 (§4.1)** The current set of rows in the tree equals the snapshotted set. There are no extra members.

- **R075 (§4.1)** Every snapshotted external target exists at its exact address and row identity.

- **R076 (§4.1)** Every Card write and Lock change is serialized through the existing edit lock.

- **R077 (§4.1)** No client changes the Lock except the owner through the GUI route.

- **R078 (§4.1)** An integrity fault does not unlock the tree. Checks continue to protect its root address, retained member row identities, and retained pin targets until the owner explicitly unlocks it.

- **R088 (§4.4)** For members sorted by address using a deterministic, locale-independent string order, hash UTF-8 bytes of:

- **R089 (§4.4)** Seal framing is UTF-8 address, one TAB, full authored fingerprint and one LF per member, sorted by address with a locale-independent order, then SHA-256 over all records.

- **R090 (§4.4)** Hash all lines with the existing SHA-256 dependency and retain the full lowercase hexadecimal digest. Display its first 12 characters. Keep the same ordering and framing in lock creation, integrity checks, dumps and future reseals. The display prefix is a visual identifier, not a replacement for full-digest comparisons.

- **R091 (§4.4)** Changing a note or generated Reverse links leaves the seal unchanged. Unlocking and relocking creates a new Lock id and history entry even when the resulting seal is the same.

- **R092 (§5.1)** Use the proposal's columns, with one active-root index suited to the destination lookup. Do not add columns to `cards`. In particular, do not store a second mutable `locked` boolean on each Card.

- **R093 (§5.1)** Schema contract: locks retains integer lock_id/root_row, root_address/tree/Workspace, optional bounded note, locked_at, full range_fingerprint, member_count at least one and optional unlocked_at; one partial unique root_address index permits one active Lock. lock_members has lock_id/row_id composite identity, address/authored_fp and a row index; lock_pins has lock_id/source_row/target_row composite identity, target_address and target index. lock_events has integer event_id, lock_id, action restricted to lock/unlock/note/reseal/integrity-fault, UTC at and canonical-JSON detail plus Lock/event ordering index. Foreign keys link lock tables, not cards.

- **R094 (§5.1)** Make creation idempotent in the migration function. The SQL above specifies the resulting schema; the initializer must also validate an existing lock-schema generation rather than trusting `IF NOT EXISTS` to prove compatibility. Store `metadata.tree_lock_schema_version = '1'` only after all tables, indexes, and triggers are installed successfully. An unsupported version refuses writes without attempting a repair.

- **R095 (§5.1)** There are deliberately no foreign keys to `cards`: Card-table rebuilds must not cascade-delete the Lock's evidence. Foreign keys between lock tables can be enforced on application connections. `locks` rows and events are never removed by an ordinary Card delete or `--all`.

- **R096 (§5.1)** Every active lookup joins `locks` with `unlocked_at IS NULL`. The unlock transaction removes member and pin rows, but the active predicate makes the meaning explicit and prevents stale historical rows from accidentally protecting reused Card row numbers.

- **R097 (§5.1)** Validate notes in Rust as at most 500 Unicode scalar values, reject embedded NUL, and map an empty note to `NULL`. Preserve accepted note text. Use UTC RFC 3339 timestamps, as in existing receipt SQL; order history by `event_id` so equal timestamps have a stable order.

- **R106 (§5.3)** Add the `functions` feature to the existing `rusqlite` dependency; no new runtime dependency is needed. Register these deterministic functions on every connection that can touch protected Cards:

- **R107 (§5.3)** SQL function contracts: deterministic zt_authored_fp consumes text, BibTeX, Card type, Model facet, emphasis, Model level, Hub level and Math form using existing authored framing; deterministic zt_tree_root consumes the address and returns the actual Topic/Literature root, with internal moving placeholders returning NULL.

- **R108 (§5.3)** Factor the existing authored-fingerprint byte framing into a pure helper used by both `card::authored_fingerprint` and `zt_authored_fp`. Preserve its output. The SQL wrapper must not query SQLite or call a loader recursively. Validate raw metadata values instead of using a decoder's fallback for an invalid enum. Parsing or hashing failure aborts the statement.

- **R109 (§5.3)** `zt_tree_root` uses the existing address rules and returns `3/0` for every member of Topic 3 and `Smith2024` for every member of that Literature tree. It returns SQL NULL for the existing internal `__moving__...` placeholders, which are never valid final Card addresses. Other address validation remains the responsibility of the writers. Registering functions does not install schema or run a migration; keep connection opening free of writer-lock recursion.

- **R110 (§5.3)** Audit direct `Connection::open` uses, including bootstrap and tests. Read-only backup connections need no Card writes, but any test or migration that executes a protected UPDATE needs the functions. An external SQLite connection without the fingerprint function cannot update a locked Card's text, including its Reverse links. That is an intentional backstop limitation, not an alternate client.

- **R111 (§5.4)** Retain the six trigger names from the proposal. Use the direct active-root lookup for insertion and move-in instead of the proposal's parent lookup. Include Card row identity and kind in member protection. Representative predicates below define the implementation; use the same active-Lock predicate in all six.

- **R112 (§5.4)** Six-trigger contract: lock_no_delete rejects active member deletion; lock_no_change rejects changed row identity, location/key, Workspace, root-kind flags or candidate authored fingerprint different from the captured baseline; lock_no_insert and lock_no_move_in reject the final address root matching any active Lock; lock_pin_no_delete rejects active target deletion and lock_pin_no_move rejects changed target address/key/row/Workspace/kind. Each lookup joins active Locks; temporary mapping names cannot bypass final destination checks; generated-only updates retain authored identity and pass. SQL ABORT must lead callers to abandon their enclosing transaction.

- **R113 (§5.4)** `lock_no_change` compares the candidate authored fingerprint to the sealed baseline. A Reverse-link refresh passes when the authored fields match. Every actual address change of a locked member or pin fails, including the first temporary rename in a mapping.

- **R114 (§5.4)** A move of an unlocked subtree into a locked tree fails on a final destination row even if its parent is still temporarily detached. The application gate detects the whole forbidden mapping before any row is detached. A trigger abort still rolls back the operation transaction; SQLite `RAISE(ABORT)` by itself only aborts a statement, so callers must propagate the error and abandon the transaction.

- **R115 (§5.4)** Do not add triggers on `links`, `metadata.gui_spreads`, or ordinary reads. Do not introduce a public trigger-disable switch. Tests must distinguish an expected application refusal from a raw trigger abort. A normal client reaching one of these trigger messages exposes a missing application check.

- **R116 (§6.1)** Names below are proposed crate-private interfaces. `&Connection` permits use with an existing `Transaction`; these functions never acquire another edit lock or commit their caller's work.

- **R117 (§6.1)** `register_sql_functions(conn)` — Register pure SQL wrappers only.

- **R118 (§6.1)** `ensure_schema_tx(conn)` — Install/validate version-1 tables and indexes.

- **R119 (§6.1)** `ensure_triggers_tx(conn)` — Install the six canonical triggers at a known migration boundary.

- **R120 (§6.1)** `plan(conn, root, note) -> LockPlan` — Resolve root, members, pins, warnings, seal, and plan hash from one view.

- **R121 (§6.1)** `lock_tx(conn, plan) -> Lock` — Insert Lock, members, pins, event, and one revision advance.

- **R122 (§6.1)** `unlock_tx(conn, lock_id)` — End exactly that active Lock, remove its live snapshots, append event, advance revision.

- **R123 (§6.1)** `note_tx(conn, lock_id, note)` — Change note and append event/revision only when note changes.

- **R124 (§6.1)** `ensure_unlocked_tx(conn, change)` — Return a typed `Refusal` for forbidden Card effects.

- **R125 (§6.1)** `read_view(conn, rows) -> LockView` — Bulk annotations for selected Cards, or every Card in a full Snapshot.

- **R126 (§6.1)** `check(conn) -> Integrity` — Pure integrity comparison; no repair or reseal.

- **R127 (§6.1)** `record_faults_tx(conn, integrity)` — Persist newly observed faults under the edit lock.

- **R128 (§6.1)** `export(conn) -> Value` — Versioned dump representation from the caller's read view.

- **R129 (§6.1)** Use a small `Change` enum that borrows existing operation data:

- **R130 (§6.1)** The borrowed Change variants are Edit(row), CreateAt(final address), Delete(existing plan), Mapping(final old-to-new map) and LinkRewrites(map); they reuse existing operation data and caller-owned transactions.

- **R131 (§6.1)** `CreateAt` checks the final address, rather than an arbitrary ancestor supplied by a caller. `Delete` has access to the requested scope as well as the rows, so an empty keep-root operation can still be refused. A mapping checks source members, pin targets and destination trees. Link rewrite checking uses the existing Link parser/index to find authored changes in locked sources.

- **R132 (§6.1)** There is no separate policy object per transport. `Refusal` implements `Error`, `Display`, and a JSON conversion, following the existing typed batch conflict/read-limit pattern.

- **R133 (§6.2)** The unconfirmed GUI request acquires the ordinary edit lock with existing GUI patience, opens a database transaction, and computes a plan. It then closes that transaction and releases the edit lock before the owner reads the dialog. Do not hold a writer mutex while the dialog is open.

- **R134 (§6.2)** The plan includes:

- **R135 (§6.2)** Archive identity and revision, root row/address, Zone, Workspace, and requested note.

- **R136 (§6.2)** Root and Regular Card counts, full seal, and full member facts used to compute it.

- **R137 (§6.2)** Every distinct Pinned target with the locked source Cards that pin it; target count is distinct targets, not the number of source/target pairs.

- **R138 (§6.2)** Every existing Broken link with source and target addresses, plus the exception in §4.3.

- **R139 (§6.2)** `confirm: "lock"` and `plan_hash`.

- **R140 (§6.2)** Use canonical JSON and the existing SHA-256 dependency for `plan_hash`. Bind it to the action, archive identity/revision, root identity, note, member baseline, pins and warnings. The confirmed request supplies the word, hash, root, and note. Under the edit lock and a new transaction, compute the plan again and compare the hash. A mismatch returns `plan_changed` with a fresh plan and no write. The GUI keeps the note and asks the owner to review again. Changing the note in the dialog requires a refreshed plan before confirmation.

- **R141 (§6.2)** This is one deterministic check; it needs no pending-plan storage, expiry job or secret token. The normal API token remains required. A plan hash does not confer owner identity.

- **R142 (§6.3)** After a confirmed, matching plan:

- **R143 (§6.3)** Verify the root is still a Tree root and has no active Lock.

- **R144 (§6.3)** Verify indexed Links for the selected members against their authored bodies. Refuse a mismatch as an index-integrity error; do not silently take incomplete pins or run a global repair.

- **R145 (§6.3)** Insert `locks`, including the root row and baseline seal.

- **R146 (§6.3)** Insert every `lock_members` and `lock_pins` row.

- **R147 (§6.3)** Append the full initial `lock` event.

- **R148 (§6.3)** Advance `archive_revision` once and commit.

- **R149 (§6.3)** Mirror the log line and return the committed Lock.

- **R150 (§6.3)** No Card text or generated Link data is written by setting a Lock. A crash before commit leaves no partial Lock. A lost response after commit can be resolved by reading the active Lock; another unconfirmed lock request reports `already_locked` with its id and seal. It never creates a second active Lock.

- **R154 (§7.1)** The common write sequence is:

- **R155 (§7.1)** Write ordering: parse, acquire the existing edit mutex, open a transaction/re-read current facts, check Access/domain rules, check complete Lock effects, write, synchronize derived Links/Reverse links/Spreads, advance revision once and commit.

- **R156 (§7.1)** Friendly plan checks run before a confirmation prompt, editor, or destructive confirmation reply. The transaction-level checks run again before SQL. A precheck does not authorize a later write if the owner has since locked a destination. The same edit lock orders the two actions: a writer either commits before the Lock captures it, or sees the new Lock and refuses.

- **R157 (§7.1)** Retain Workspace refusals for agent attempts to write human Cards. For an otherwise allowed operation, return the Lock refusal. Do not skip tree checks when `access == Owner`.

- **R158 (§7.1)** If a low-level function accepts a previously loaded `Card`, query its current lock/root facts in the transaction. Do not trust a lock annotation retained by a browser, Session or workflow.

- **R159 (§7.2)** `main::insert_card_tx`; transaction-level successor creation — `CreateAt(final_address)`; prevents main-line append and Side successor creation.

- **R160 (§7.2)** Topic/Literature creation, including `ops::create_literature_tx` — Final root-address check. A genuinely new tree is allowed; an active orphaned Lock is not bypassed by recreating its address.

- **R161 (§7.2)** `main::update_card_text`; `ops::edit_tx` — `Edit(row)` before text or metadata steps and before a no-op shortcut.

- **R162 (§7.2)** `batch::owned_paragraph` — `Edit(row)` before splicing authored bytes, or recording a new ownership/adoption mutation on that Card.

- **R163 (§7.2)** `model::set`, `model::set_level`, `hub::set_level`, `math::set_form` — `Edit(row)` in each transaction-level setter, including changes implied by retyping.

- **R164 (§7.2)** Literature body editing and `update_literature_metadata` — `Edit(root)` for authored changes; complete mapping/pin/rewrite checks before a Citation-key UPDATE.

- **R165 (§7.2)** `place::apply_insert_plan` — Check existing mapping and the new final address before direct INSERT and temporary moves.

- **R166 (§7.2)** `main::apply_location_mapping_tx` — `Mapping(final_mapping)` and locked-source Link-rewrite checks before the first `__moving__` update.

- **R167 (§7.2)** `main::apply_delete_plan` — Requested scope, complete delete set, `into` rewrite mapping and actual Side compaction, before any UPDATE/DELETE.

- **R168 (§7.2)** `reply::write_tx` — The ordinary insertion guard on the human destination. The locked agent source is read and linked.

- **R169 (§7.2)** GUI hub-draft Save / file-Spread-as-Card, through existing create/edit routes — Edit guard for `how: "edit"`; final insertion guard for successor placement. `hub::draft` and `hub::precheck` remain read-only.

- **R170 (§7.2)** Reverse-link refresh / Link-index rebuild — No authored edit request; fingerprint-preserving generated UPDATEs may pass the trigger.

- **R171 (§7.2)** `zt del --all` — Explicit active-Lock check even on a damaged archive or an empty Card table.

- **R172 (§7.2)** Use the same checker for the shell and Session plan display. Do not write equivalent, independent SQL in each command handler. During implementation, search every `INSERT INTO cards`, `UPDATE cards` and `DELETE FROM cards` and classify it as one of these paths, a generated refresh, a migration, or an isolated fixture.

- **R173 (§7.3)** Check the full existing old-to-new mapping, not only the Card the user selected. A move before a Card may rename a later Direct chain and its Side branches. A delete can compact siblings. An insertion can shift the Card which a locked source pins even when the inserted Card itself is in an unlocked tree.

- **R174 (§7.3)** For each effective mapping entry where old and new differ:

- **R175 (§7.3)** Refuse a source row which is a locked member.

- **R176 (§7.3)** Refuse a source row which is a Pinned target.

- **R177 (§7.3)** Refuse a destination inside an active Locked tree.

- **R178 (§7.3)** Check whether rewriting a macro in any locked source would change that source's authored body.

- **R179 (§7.3)** Use the final map, not the temporary placeholder names, for structured error addresses. Preserve the existing order of `into` re-pointing followed by Side compaction. Check both effects before either starts. Do not silently omit forbidden compaction, change a destination, or create a new tree to make a rejected request succeed.

- **R180 (§7.4)** A delete of a pinned Card is refused even when the locked Link's source is absent from the delete set, and even with `into`. If the source is also selected, its own membership refusal still blocks the operation. There is no deletion order that releases a Lock implicitly.

- **R181 (§7.4)** A target whose root is pinned but whose Regular Cards are not pinned may be emptied with keep-root. Conversely, one pinned Regular Card is enough to refuse emptying that tree. Use the actual delete set. Do not freeze a target's whole tree merely because one Card is pinned.

- **R355 (§18.1)** T01 — Lock root-only and populated trees; attempt to lock a Regular Card — Root included; exact membership; Regular anchor refused

- **R356 (§18.1)** T02 — Edit title/body/BibTeX, owned paragraph, type/facet/emphasis/level/Hub level/form — Structured refusal before write; all authored fingerprints unchanged

- **R357 (§18.1)** T03 — Create direct/side/insert successor, including append at last main-line Card — Refused; no Card, allocation, receipt or Link-index residue

- **R358 (§18.1)** T04 — Move/insert/compact a member; move an unlocked subtree into the tree — Refused from complete final plan; no temporary Locations persist

- **R360 (§18.1)** T06 — Move/delete/rename pinned target, ancestor subtree or entire target tree; delete with `into` — Refused with source and Locked-tree explanation

- **R364 (§18.1)** T10 — Raw registered-connection SQL bypasses friendly gate for each prohibited effect — Trigger refuses; full enclosing transaction rolled back

- **R365 (§18.1)** T11 — External connection lacks SQL wrapper and updates locked text — Statement fails; no authored or generated text change

- **R366 (§18.1)** T12 — Move a subtree with parent temporarily detached; append after main line; malformed prefix/case fixtures — Destination-root check catches locked tree without confusing other trees

- **R376 (§18.1)** T22 — Lock/unlock/note/installation fails before commit; committed response lost — No partial records; read resolves uncertain outcome; unique active root retained

## Acceptance criteria

- [ ] **S01-AC01** Only actual Topic and Literature roots, including root-only trees in either Workspace, can anchor a Lock. Regular, nonexistent, already locked or inconsistent roots are refused; tree membership respects existing case, prefix, Workspace and Zone rules.
- [ ] **S01-AC02** Use one tree_lock module, the existing SQLite archive, writer mutex, transaction helpers, Link index, revision and authored-fingerprint algorithm. Retain the four lock tables and indexed membership/pin checks; add no Card boolean, separate service, new permission framework or second writer mutex.
- [ ] **S01-AC03** Persist all specified Lock, member, pin and event fields, checks, primary keys, active-root uniqueness and supporting indexes. Validate note length as at most 500 Unicode scalar values, reject NUL, and store empty notes as NULL. UTC timestamps and event-id ordering are deterministic.
- [ ] **S01-AC04** Freeze exact member row/address/Workspace/kind/authored identity and prevent every authored edit, BibTeX/type/facet/emphasis/model-or-Hub-level/Math-form change, insertion, incoming move, deletion and effective address change. The owner has no exemption; ordinary agent Workspace restrictions still apply.
- [ ] **S01-AC05** Guard every storage writer, including direct shell/Session setters, Literature creation/rename, placement INSERT, owned-paragraph/adoption, Reply destination, hub-draft Save, complete delete/repoint/compaction effects and every final mapping destination. Plan prechecks precede prompts/editors; transaction checks repeat before SQL and no-op shortcuts.
- [ ] **S01-AC06** Snapshot distinct cross-tree source/target pairs from verified authored Link/index facts. Pins prevent target movement, Citation-key rename, deletion and into re-pointing. Full mappings include shifted chains, Side branches, temporary renames and target ancestors; no forbidden compaction/destination is silently substituted.
- [ ] **S01-AC07** Register deterministic SQL fingerprint and final-tree-root functions on every writable/bootstrap/migration connection without recursive queries or migration-on-open. Share existing fingerprint framing exactly; invalid raw metadata/parsing aborts. Keep row identities and kinds separately protected.
- [ ] **S01-AC08** Install all six named triggers: member delete/change, tree insert/move-in, pin delete/move. Destination lookup uses the active root rather than parent update order; ordinary reverse-only refresh is allowed. A normal client must receive the application refusal before a trigger; a trigger error abandons the enclosing transaction.
- [ ] **S01-AC09** The seal is the full lowercase SHA-256 of locale-independent address-sorted UTF-8 address/TAB/authored-fingerprint/LF records; show its first 12 characters. Preserve existing authored normalization and exclude generated Reverse links without inventing a second receipt fingerprint.
- [ ] **S01-AC10** Token-guarded GUI Lock/unlock routes plan first and confirm second with a stateless canonical plan_hash bound to action, archive identity/revision, root/Lock identity, note, member/pin/warning facts. Recompute under the writer mutex; stale or missing hashes never commit. Release the mutex while the owner reads a dialog.
- [ ] **S01-AC11** The Lock plan presents complete members/counts, distinct pinned targets and their sources, full seal, note and preexisting Broken-link warnings. Originally broken addresses remain unpinned for that baseline; a later resolving target requires relocking to join the promise.
- [ ] **S01-AC12** Lock commit atomically persists snapshots/event and advances the revision once without editing Card/Link data. Unlock ends the exact active Lock, removes only its live member/pin rows and retains history. Lost responses are resolved by reading state; no duplicate active Lock is created.
- [ ] **S01-AC13** No MCP, shell or Session lock-control operation is added. Owner authority remains the GUI-token policy selected in D4, without a secret, strict human authentication claim, automatic Lock or force bypass.
- [ ] **S01-AC14** Public GUI/shell/Session/MCP and raw-SQL fixture tests exercise the complete prohibited-write matrix in both tree kinds and Workspaces as Access permits, compound mapping/temporary-rename cases, cancellation/races/rollback, external connections missing SQL functions and crash-before/after-commit behavior.
- [ ] **S01-AC15** Update the storage, Card-operations and Cards/Workspaces/Links block docs and initial glossary entries with their code; later issues refine their own surfaces without weakening these foundation invariants.

**Verification seam:** Real token-guarded GUI routes plus real CLI/Session/MCP process fixtures and raw SQLite backstop tests in isolated archives; compare stored state and fingerprints, not only messages.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

None - can start immediately

## S02 / #125 — Keep locked Cards readable and linkable while preserving external pins

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Demonstrate the allowed side of the promise across the actual reader, author, Reply and navigation paths. Locking a tree must leave it useful, and pinning an external Card must preserve addressability without freezing its meaning.

**Type:** AFK. **Slice:** S02. **User stories:** US006, US007, US008, US009, US010.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R029 (§1.2)** Broken Links present at lock time — Show them before confirmation; they remain outside the baseline pin promise — Matches the proposal's explicit rule that these Links have no pin row. See §4.3.

- **R038 (§2)** Read and reference locked Cards — Preserve search, model, hub, register, Spread, and question behavior; annotate reads — 9

- **R039 (§2)** Generated Reverse links may change — Fingerprint excludes generated content; derived index/reverse refresh remains allowed — 4, 5

- **R061 (§3)** `the Reply module` — Reply writes a human Card through `create_successor_tx` and updates the agent target's Reverse links. — A locked target is allowed; a locked human destination is refused.

- **R079 (§4.2)** The seal is defined by the existing `card::authored_fingerprint`, not a new text-normalization rule. Its fields are parsed title, parsed body, BibTeX, Card type, Model facet, emphasis, effective Model or Hub level, and Math form. The existing Card parser normalizes its envelope and line endings. Therefore the promise is authored identity under that existing function, not byte-for-byte identity of the entire stored Markdown document.

- **R080 (§4.2)** The address, Workspace, row identity, and `is_topic`/`is_lit` flags are checked separately. They are not all part of the authored fingerprint.

- **R081 (§4.2)** Reverse links, Link-index rows, GUI view state, Spreads and external artifact/cache state are not authored Card changes. Updating them is permitted. The Lock does not freeze a source PDF in Scholar Archive, a source document's availability, or an external target's meaning.

- **R082 (§4.2)** Explicit edit, retype, move, create-inside and delete requests against a locked tree return a Lock refusal, including requests that would happen to be no-ops. A pure receipt replay is different: it reports an earlier result and performs no Card write (§8.2).

- **R083 (§4.3)** At lock time, parse the locked members' authored Links and resolve them using the existing Link index. Each distinct `(locked source row, existing target row)` in another tree becomes one pin. Repeated macros produce one pin; different source Cards retain separate rows so errors can explain which locked Card needs the target. Cross-Workspace Links qualify.

- **R084 (§4.3)** A pin preserves the target's address and existence. It permits edits to the target's body, title, BibTeX without a Citation-key rename, type, and model/hub/math fields if the ordinary Access and structure rules permit those edits. A target can also belong to a Locked tree, in which case its own Lock supplies the stronger restrictions.

- **R085 (§4.3)** Several Locks can pin the same target. Unlocking one releases only its pins. The target remains protected while any other active Lock pins it. Pins do not propagate: if locked A links unlocked B, and B links C, only B is pinned by A. No transitive dependency graph is needed.

- **R086 (§4.3)** Existing Broken links are warnings, not a reason to refuse locking. Retain their source and target addresses in the lock event and confirmation plan. They create no pin. If one of these addresses becomes live later, the baseline does not silently gain a new pin; unlocking and locking again captures the now-resolving Link. The GUI must explain this exception next to the warning. The promise concerns targets that resolved when the owner accepted the Lock.

- **R087 (§4.3)** Only actual authored Links qualify. A quoted `[[address]]` inside a code fence, generated Reverse text, an artifact, a Document link, or `data-zt` chart navigation does not independently create a pin. Use existing Link parsing; do not add a regex with different rules.

- **R359 (§18.1)** T05 — Edit a pinned target without renaming it — Allowed when ordinary rules allow; locked source seal unchanged

- **R361 (§18.1)** T07 — Two Locks pin one target; release one, then the other — First release retains protection; second releases it

- **R362 (§18.1)** T08 — Add/remove incoming Link; Reply to locked agent Card; change generated Reverse links — Allowed at unlocked source/destination; locked authored fingerprints and seal unchanged

- **R363 (§18.1)** T09 — Reply/hub draft/Spread filing attempts to write inside Locked tree — Refused atomically; no fallback Topic or partial draft Card remains

- **R373 (§18.1)** T19 — Preexisting Broken link; target later appears — Warning retained; baseline pin exception explicit; relock captures target

## Acceptance criteria

- [ ] **S02-AC01** Every read, search, export, model/hub/register/question path, Spread/Table placement and ordinary navigation continues to include locked Cards with unchanged ranking/selection semantics.
- [ ] **S02-AC02** Create or remove incoming Links from unlocked sources and write a valid human Reply to a locked agent Card. Regenerate Reverse links and derived Link rows while every member authored fingerprint and range seal remains unchanged.
- [ ] **S02-AC03** Permit unlocked Pinned-target authored/metadata edits, including BibTeX changes without key rename, when Access/domain rules allow. A target which also belongs to a Locked tree obeys its own stronger Lock.
- [ ] **S02-AC04** Pins are distinct source/target pairs, deduplicated per repeated macro, cross-Workspace where applicable and non-transitive. Two active Locks can pin one target; releasing one retains the other protection.
- [ ] **S02-AC05** Only authored Link macros recognized by the existing parser qualify. Fenced examples, generated Reverse text, artifacts, Document links and chart navigation do not independently pin a Card.
- [ ] **S02-AC06** Persist/show the source and target of original Broken-link warnings; later target creation does not silently add a pin, and relocking captures a now-live target. Explain that target meaning and external document/artifact/cache availability are outside the seal promise.
- [ ] **S02-AC07** A Reply or hub/Spread draft aimed at a locked destination refuses atomically, retains the draft and creates no fallback Topic or partial Card. A locked source alone is not a reason to reject the operation.
- [ ] **S02-AC08** Verify actual Link-index/Reverse-link correctness after allowed writes, multi-Lock pin release and both source/destination race cases; update the relevant Cards/Links, GUI and Card-operations docs in the same commit.

**Verification seam:** Existing public Reply and create/edit APIs, full/scoped reads, Link integrity command, model/hub/search fixtures and stored authored fingerprints.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)

## S03 / #126 — Expose consistent Lock state through every read and normal GUI refresh

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Return consistent current lock/pin facts on full and lean Card projections and make an open browser notice a Lock-only change through its existing refresh lifecycle.

**Type:** AFK. **Slice:** S03. **User stories:** US018, US019, US043.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R059 (§3)** `the scoped-read module`, `the shared Card-operations module` — MCP has scoped and lean text readers as well as full Snapshots. — Update both projections. A Snapshot-only addition would miss `get_cards(view: "text")`.

- **R060 (§3)** `the GUI HTTP module`: `Snapshots`; `Snapshot::fingerprint` — SQLite `data_version` invalidates the cached Snapshot; the GUI's displayed fingerprint currently hashes Card data. — Lock-only changes must also change the fingerprint seen by the browser.

- **R201 (§9.1)** Return these additive fields on full Card objects and Card summaries:

- **R202 (§9.1)** Each Card annotation is lock null or an object containing lock_id, root-address tree and UTC since, together with a deduplicated pinned_by array; an unpinned Card has an empty array.

- **R203 (§9.1)** An unlocked Card has `lock: null`. An unpinned Card has `pinned_by: []`. A target linked by several members of one Locked tree lists that root once. A Card can have both a Lock and nonempty `pinned_by`. Keep existing Card `tree` and `root` fields unchanged.

- **R204 (§9.1)** Tree rows from `list_trees` and GUI `trees()` add `locked: true|false`. The existing `writable` field must become truthful for its client: `access.can_write(workspace) && lock == null`. A pin alone does not make a Card uneditable; clients disable movement/deletion separately.

- **R205 (§9.2)** Keep `Card` as the stored Card model. Add a small request/Snapshot `LockView`, with maps from row identity to active Lock information and pinning roots. Bulk-load it from the same connection and read transaction used to load the Cards. One `decorate_card` helper attaches the fields.

- **R206 (§9.2)** Full Snapshots load all relevant lock facts once. Scoped MCP reads load annotations for their selected/context rows using the member-row and target-row indexes. Do not reparse every locked tree, recalculate seals, or issue a separate query for every rendered Card.

- **R207 (§9.2)** Wire this into full `Snapshot::summary` and the lean `read::text_facts` path. The constructor for a database-backed Snapshot must receive its Lock view explicitly; reserve an empty view for isolated pure fixtures. This avoids accidentally representing a real archive's lock state as empty.

- **R208 (§9.2)** Cover `get_card`, both `get_cards` views, `get_tree` with and without bodies, search rows, GUI summary/detail/navigation, and exported title rows used in destination planning. Model/hub results that embed Card summaries use that same helper. Bare address strings do not need to become full Card objects. `ask_box` and entry search retain their selection/ranking rules; any Card annotations they return are descriptive only.

- **R209 (§9.3)** A successful lock, unlock, changed note, reseal or newly recorded fault advances the existing archive revision once. It does not rebuild `links`. Member/pin insertion alone is not an authored source change.

- **R210 (§9.3)** Have the Snapshot retain the read view's archive revision, and include that revision in its existing GUI fingerprint. SQLite `data_version` already invalidates the server's cached Snapshot when another connection commits; the changed fingerprint makes the browser notice a Lock change even when every Card's text is identical. Read Cards, annotations and revision in one SQLite read transaction. Do not add another polling loop or a persistent lock-state cache.

- **R211 (§9.3)** This revision is a concurrency marker, not an authored fingerprint. Reply and other authored-base checks continue to use their current authored identity. A new incoming Link can update a locked Card's Reverse section and archive revision without changing its seal.

- **R371 (§18.1)** T17 — `get_card`, both bulk views, tree/search/export/model/hub/GUI projections — Consistent `lock`, `pinned_by`, `locked` and client `writable` in one revision

- **R372 (§18.1)** T18 — Only lock/note/fault metadata changes — Existing browser heartbeat observes new revision/fingerprint without a Card edit

## Acceptance criteria

- [ ] **S03-AC01** Full Card objects and summaries return lock null or lock_id/root-address/since and a sorted deduplicated pinned_by array. A Card may have both; existing Card tree-id/root meanings do not change. Tree rows add locked boolean.
- [ ] **S03-AC02** Cover get_card, both get_cards views, get_tree with/without bodies, search/export-title rows, GUI detail/summary/navigation and embedded model/hub Card summaries. Bare addresses need not expand into full objects; retrieval/ranking remains unchanged.
- [ ] **S03-AC03** Client writable means Workspace permission AND no active Lock. A pin alone leaves authored editing possible while movement/deletion are constrained separately.
- [ ] **S03-AC04** Keep Card as the persisted Card model and use one explicit bulk LockView/decorator in the same SQLite read transaction as Cards/revision. Full and scoped readers must not silently receive an empty production view or issue one query per rendered Card.
- [ ] **S03-AC05** Use existing member/target indexes for scoped annotations; ordinary reads do not hash all locked trees, rebuild Links or create another persistent lock cache.
- [ ] **S03-AC06** Lock, unlock, changed note, reseal and newly recorded faults advance the existing revision once without a Link rebuild. Include that captured revision in the Snapshot/browser fingerprint so metadata-only commits are observable through existing data_version invalidation.
- [ ] **S03-AC07** Authored-base checks retain their existing fingerprint semantics; generated Reverse changes can change a revision without changing a seal. Public tests compare lean/full views at one revision and observe metadata-only browser refresh.
- [ ] **S03-AC08** Update Card-operations, MCP and GUI block docs with their code and document additive response compatibility and current client writable meaning.

**Verification seam:** Real MCP single/bulk/text/tree/export/search calls, GUI snapshot/overview polling and independent full-reader oracles from one archive revision.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)

## S04 / #127 — Edit Lock notes and inspect retained owner history

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Give the owner a confirmed note edit and a useful complete history panel which remains understandable after unlock, rename or deletion of an old tree.

**Type:** AFK. **Slice:** S04. **User stories:** US014.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R025 (§1.2)** Note confirmation — Use the same plan/confirm exchange, with `confirm: "note"` — The proposal's Authority section requires confirmation for changing a note, although its route sketch omits that field.

- **R098 (§5.2)** `detail` is deterministic JSON built with the existing JSON library. It is an audit record, not a second mutable state machine.

- **R099 (§5.2)** `lock` — Root address, Workspace, note, full seal, member count, member address/row/fingerprint list, pin source/target/address list, preexisting Broken-link warnings

- **R100 (§5.2)** `unlock` — Root, prior seal, prior note, member/pin counts, and the released Lock id

- **R101 (§5.2)** `note` — Root, previous note, new note; no duplicated member snapshot

- **R102 (§5.2)** `reseal` — Migration identifier, old/new seal and changed-member baseline details

- **R103 (§5.2)** `integrity-fault` — Stable fault identifiers and actual/expected facts required to explain them

- **R104 (§5.2)** The initial `lock` event retains the snapshot because unlocking deletes the live member/pin rows. History and dumps must remain understandable after the unlocked tree is renamed or deleted. Use the event's captured addresses instead of joining historical row ids to today's Cards.

- **R105 (§5.2)** Write the authoritative event in the same SQLite transaction as the change. After commit, mirror one concise line to `zt.log`. A filesystem log failure cannot undo the committed Lock; report the logging problem separately. The GUI reads SQLite history.

- **R151 (§6.4)** Unlock planning names the active `lock_id`, tree, note, seal, member count and pin count. Its confirmation hash binds that id and the current revision, so an old dialog cannot unlock a later Lock on the same tree. On confirmation, end the active Lock, delete only that Lock's live member and pin rows, append `unlock`, and advance the revision in one transaction. Historical `locks` and events remain.

- **R152 (§6.4)** Unlock remains available on a faulted Lock. Resolve it from its retained Lock record even if the root is missing. The GUI's lock list can therefore release an orphaned Lock after the owner has reviewed the fault; it does not require opening a missing Card.

- **R153 (§6.4)** Note changes identify the active Lock in the same way. A changed note adds a `note` event and revision, with no seal or member change. An identical note returns success without another event or revision. An absent or ended Lock returns `not_locked`; it does not edit historical notes.

- **R213 (§10.1)** `GET /api/locks` — No body — Active Locks, historical Lock/event groups and current integrity result. Include ended Locks so history survives unlock.

- **R214 (§10.1)** `POST /api/lock` — `{root, note?}` — `{root, note?, confirm:"lock", plan_hash}` → committed id, root, counts and seal

- **R216 (§10.1)** `POST /api/lock/note` — `{tree, lock_id, note}` — Same plus `confirm:"note", plan_hash` → id, root and note

- **R218 (§10.1)** For `GET /api/locks`, group events by Lock id and root identity, not only the current root address. Order Locks by creation/id and events by event id. Present long lists in a scrollable panel. Return complete lists for this local API rather than silently cutting off pins or history. If a future measured size problem needs paging, add explicit paging then; do not introduce a cursor system in this release.

## Acceptance criteria

- [ ] **S04-AC01** Provide GET /api/locks with active and ended Lock/event groups and integrity facts, ordered by Lock identity and event id. Group by retained Lock id/root identity rather than joining historical row numbers to current Cards.
- [ ] **S04-AC02** POST /api/lock/note plans first and requires confirm note plus plan_hash for that active lock_id. Recompute under the writer mutex; an ended/absent Lock returns not_locked and stale confirmation returns plan_changed without writing.
- [ ] **S04-AC03** Enforce 500 Unicode scalar values, NUL rejection and empty-to-NULL storage. Preserve accepted note text. An identical note succeeds without another event/revision; a changed note leaves the seal/member/pin baselines unchanged.
- [ ] **S04-AC04** The initial lock event retains complete member and pin snapshots plus original Broken-link warnings; unlocking deletes live snapshots but preserves the historical record. Unlock/note/reseal/fault events retain their specified previous/new evidence.
- [ ] **S04-AC05** Write authoritative SQLite events atomically with state/revision. Mirror a concise filesystem log after commit; a log failure reports separately and cannot pretend the committed Lock was rolled back.
- [ ] **S04-AC06** The GUI panel exposes note, seal, members/pins, ended Locks and complete event history in scrollable/expandable views without silently truncating lists or adding an unnecessary cursor system.
- [ ] **S04-AC07** Test note bounds/Unicode/NUL, current/stale/identical note outcomes, multiple Lock generations with the same root/seal, a deleted historical root and an after-commit log failure; update GUI/storage docs.

**Verification seam:** Token-guarded note/list routes, actual retained SQLite history and GUI panel interaction; no new low-level mock-only interface.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)

## S05 / #128 — Review GUI Lock plans and preserve drafts when restrictions change

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Finish the owner-facing interaction around the safe core: recognizable bands/seals/pins, complete current plans, disabled prohibited actions and drafts that survive a concurrent Lock.

**Type:** AFK. **Slice:** S05. **User stories:** US011, US012, US016, US017, US019.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R043 (§2)** GUI appearance and controls — Band, seal, disabled write controls, pin explanations, history panel — 10

- **R212 (§10.1)** All routes use the existing GUI token check and existing request-size handling. Only the GUI handler calls lock-control functions; do not add these actions to the general MCP writer dispatch.

- **R215 (§10.1)** `POST /api/unlock` — `{tree, lock_id}` — Same plus `confirm:"unlock", plan_hash` → id, root and `unlocked_at`

- **R217 (§10.1)** Unconfirmed POSTs return `applied: false`, the expected confirmation word, plan hash, structured plan facts and human-readable `lines`. A confirmed request without its hash is invalid. A stale hash returns the renewed plan and preserves the form contents. The server recomputes all facts; counts, fingerprints or member arrays submitted by a browser are never authoritative.

- **R219 (§10.2)** Reuse the cabinet's tree menu and existing dialog treatment. A small band across the tree stack and a seal mark on the root communicate the proposal's physical-card metaphor. The state also has text, so it does not depend on color or a decorative icon.

- **R220 (§10.2)** **Lock tree** opens the plan dialog: tree/title/Workspace, member count, seal, pin count, note, expandable complete target/source list, and Broken-link warnings.

- **R221 (§10.2)** The confirm button is enabled only for a current plan. Updating the note renews the plan.

- **R222 (§10.2)** After success, the cabinet and all open copies of its Cards show locked state through the normal refresh. Display the 12-character seal; make the full digest available in the panel.

- **R223 (§10.2)** **Unlock tree** shows the exact Lock and what its release permits. Confirmation removes the band while retaining history.

- **R224 (§10.2)** **Edit lock note** changes the note through its own confirmation. It does not reseal the tree.

- **R225 (§10.2)** For a locked Card, edit, type/level/form, BibTeX, move, delete, insert and successor controls remain visible but disabled. Keyboard handlers and alternate context-menu actions must honor the same state. The explanation names the Locked tree and directs the owner to its unlock action.

- **R226 (§10.2)** For an unlocked Pinned target, editing stays available. Movement and deletion controls explain which Locked trees pin it and which source Cards hold the Links. Creating a successor may still be allowed; the server checks whether its actual mapping would move some other pinned Card.

- **R227 (§10.2)** A locked Card remains selectable, readable, linkable, addable to the Table, and usable as the target of a Reply. Reply destination controls exclude locked human Topics or explain a refusal without discarding the draft. An unavailable destination is not permission to unlock or silently change the owner's chosen placement.

- **R228 (§10.3)** An editor opened before a tree is locked may hold unsaved text. Keep that draft visible. A refused Save displays the structured Lock error and offers navigation to the tree's lock panel; it does not discard the draft or automatically retry after an unlock.

- **R230 (§10.3)** The normal token protects these routes exactly as it protects other GUI changes. The proposal accepts that a deliberate local process could obtain it. Do not claim strict authentication of a human or resistance to direct SQLite tampering.

- **R367 (§18.1)** T13 — Lock/unlock/note confirmation sees changed revision, root identity or Lock id — `plan_changed`; no stale action committed

## Acceptance criteria

- [ ] **S05-AC01** Cabinet menus offer Lock tree, Unlock tree and Edit lock note. Review root/title/Zone/Workspace, member and distinct target counts, full/short seal, source/target details, note and original Broken-link exception before confirmation.
- [ ] **S05-AC02** Refresh the stateless plan after a note edit or changed revision; preserve form contents and require renewed review. An old unlock dialog cannot act on a later Lock even if root/seal are reused; server-submitted counts/baselines are never trusted.
- [ ] **S05-AC03** Use the existing dialog/cabinet aesthetic with a physical stack band and seal, plus visible text. Lock state must not depend on color or decorative icons; full digest remains available.
- [ ] **S05-AC04** Disable edit, type/level/form, BibTeX, move, delete, insert and all successor actions on a locked Card, including keyboard and alternate context-menu entry points. Explain the blocking tree and GUI unlock action.
- [ ] **S05-AC05** For an unlocked Pinned target, keep editing available and explain move/delete restrictions with pinning trees/source Cards. Do not ban a harmless successor by treating a pin as a whole-tree Lock; server checks actual structural effects.
- [ ] **S05-AC06** Keep selection, reading, linking, Table placement and Reply available for locked sources. Locked human Reply destinations are excluded/explained; do not silently change the chosen placement or unlock anything.
- [ ] **S05-AC07** A preexisting editor/draft remains visible when Save is refused after another client locks the destination. Keep text/placement, show the structured refusal, allow navigation to the lock panel and do not discard or automatically retry the draft.
- [ ] **S05-AC08** Verify real served JavaScript/UI, context menus, keyboard actions, lock-only refresh and stale-dialog/draft races using isolated archives; update GUI documentation in the same commit.

**Verification seam:** Existing browser/JavaScript behavior tests backed by real GUI routes; visually inspect bands/seals, explanations and retained drafts without modifying the owner archive.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)
- [S03 / #126](https://github.com/liudesheng9/zet/issues/126)
- [S04 / #127](https://github.com/liudesheng9/zet/issues/127)

## S06 / #129 — Detect exact integrity faults and keep owner recovery reachable

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Expose a complete read-only integrity comparison and persistent fault evidence, with a broken-seal presentation and a GUI recovery path even when the locked root has vanished.

**Type:** AFK. **Slice:** S06. **User stories:** US015, US021, US035, US036.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R026 (§1.2)** Integrity scan — Compare exact member and pin facts and trigger definitions, not only stored member hashes — Detects an extra Card, changed Workspace, missing pin, or trigger replaced under the same name.

- **R027 (§1.2)** Integrity recording — A pure scan returns faults; a caller under the edit lock records each new fault once — Reads do not recursively write or spam the history.

- **R041 (§2)** Integrity and history — Startup/full checks, persistent fault events, broken seal, `--check-locks` — 13

- **R229 (§10.3)** The lock panel shows history and integrity faults. A fault uses a broken seal on the tree and one concise page-level line. Keep **Unlock tree** reachable from the lock list when the root itself is missing. Faults do not disable reading, hide Cards, or silently update the displayed seal to match damaged content.

- **R278 (§13.1)** `check(conn)` runs in one consistent read view and returns per-Lock faults plus schema faults. For every active Lock it verifies:

- **R279 (§13.1)** Root row exists at the saved root address, has the saved Workspace, and is a Tree root of the saved tree.

- **R280 (§13.1)** Stored member count equals the number of snapshot rows.

- **R281 (§13.1)** Every snapshot row resolves to the saved row/address and correct Workspace/kind.

- **R282 (§13.1)** Each current authored fingerprint matches its stored baseline.

- **R283 (§13.1)** The current exact tree membership equals the snapshot, including detection of extra Cards.

- **R284 (§13.1)** Recomputed range fingerprint equals `range_fingerprint`.

- **R285 (§13.1)** Every pin source is a member, every pin target still exists at its saved row/address, and the target is outside the locked tree.

- **R286 (§13.1)** The set of resolvable external authored Links, excluding the captured originally broken addresses, agrees with the stored pin set. Cross-check the relevant Link-index rows too.

- **R287 (§13.1)** All six triggers exist with the canonical definitions for the supported lock-schema version, and required tables/indexes are intact.

- **R288 (§13.1)** Do not use only inner joins: a missing member or target must produce a fault. Compare raw identity and metadata values when needed so a decoder fallback cannot hide a changed Workspace/type. Historical ended Locks need not match today's Cards and are not scanned as active promises.

- **R289 (§13.1)** Fault identifiers include Lock id, fault kind and affected row/address. Repeated scans of the same fault do not create another event. A newly observed fault adds evidence; successful comparison later does not erase a previously recorded active-Lock fault. “Healthy” requires both no current fault and no latched fault for that active Lock.

- **R290 (§13.2)** Service startup, before accepting writes.

- **R291 (§13.2)** `zt status --check-locks`, and the status path when it reports a current integrity result.

- **R292 (§13.2)** Opening/refetching the GUI lock panel.

- **R293 (§13.2)** Before a migration that can affect Cards, row identity or protection triggers.

- **R294 (§13.2)** Routine Card reads and ordinary GUI heartbeat polls do not hash every locked member. This is a local integrity check at known points, not continuous tamper monitoring.

- **R295 (§13.2)** A caller that needs to record faults takes the existing edit lock, scans/rechecks in its own transaction, appends only new fault events, advances the revision once if necessary, and commits. Keep pure `check` separate from recording so it can also be used in read-only verification and tests. A `--check-locks` invocation that cannot persist its evidence must report that recording failed; it cannot report a healthy state merely because no event was written.

- **R296 (§13.3)** On the first installation, with no existing lock schema, create tables/triggers normally. On a later startup, inspect the existing trigger catalog **before** `ensure_triggers_tx` recreates anything. Otherwise an accidentally missing trigger would be repaired and its evidence lost before the scan sees it.

- **R297 (§13.3)** For a supported schema whose protection trigger is missing or altered, record the fault and reinstall the canonical trigger in the guarded startup transaction. Retain the fault latch and show the broken seal. Do not accept writes until the required trigger backstop is present. If reinstallation fails, refuse writable startup. A content/member/pin fault with a working backstop leaves reading and GUI unlock available; the application gate still refuses protected changes.

- **R298 (§13.3)** If tables or baseline evidence are missing/corrupt, do not fabricate empty snapshots or new seals. Report the unsupported/damaged state and require restoration or a specifically designed repair. No generic “repair locks” tool is part of this release.

- **R299 (§13.4)** The owner can restore a consistent database backup, or review the fault and unlock/relock through the GUI. Relocking makes a new promise about the content then present. It does not prove that the old content survived or restore deleted Cards. The old fault events remain in history.

- **R300 (§13.4)** Never reseal in response to an ordinary failed integrity check. Reseal is reserved for an explicit trusted schema/text-representation migration (§14.2). Deliberate direct database modification or dropped trigger/schema protections remains outside the policy guarantee chosen in D4.

- **R374 (§18.1)** T20 — Tamper with member text/address/Workspace/kind, remove member, add member, alter pin or trigger — Exact fault detected; broken seal latched; repeated scan adds no duplicate event

- **R377 (§18.1)** T23 — Unlock faulted/orphaned Lock; relock after review — Owner can release retained record; old fault history remains; new Lock id created

## Acceptance criteria

- [ ] **S06-AC01** In one view verify root row/address/tree/Workspace/kind, stored member count, every member identity and authored fingerprint, exact membership including extras, and the recomputed full range seal. Missing rows are faults, not filtered inner joins.
- [ ] **S06-AC02** Verify every pin source belongs to the snapshot, each target retains row/address outside the tree, and current external authored/index Links agree with the stored baseline after excluding captured originally broken addresses. Raw decoder fallbacks must not hide corrupt identity/metadata.
- [ ] **S06-AC03** Verify all six canonical trigger definitions and required lock schema/index facts, not just trigger names. Ended Locks are historical evidence, not active promises against today's Cards.
- [ ] **S06-AC04** Keep pure check separate from recording. Under the writer mutex/transaction recheck and append newly observed stable fault identifiers/evidence once; advance revision once only when recording changes state. Repeated scans do not spam events.
- [ ] **S06-AC05** A successful later comparison does not erase a latched active-Lock fault. Faulted Locks stay active and continue protecting retained root/member/pin facts; no automatic baseline refresh or unlock occurs.
- [ ] **S06-AC06** Run checks at startup before writable service, status/full-check and lock-panel refresh, and before relevant migrations; ordinary Card reads/heartbeats do not continuously hash every member.
- [ ] **S06-AC07** Missing/altered supported triggers are detected and recorded BEFORE guarded canonical reinstallation; keep the fault latch after repair. Refuse writable startup if backstop repair fails, and never fabricate missing/corrupt baseline tables or evidence.
- [ ] **S06-AC08** GUI displays a broken seal and concise page-level line while keeping reading/history and owner unlock accessible. A faulted/orphaned Lock can be planned/unlocked by retained Lock id from the list without requiring an existing root Card.
- [ ] **S06-AC09** Restoration of a consistent backup or explicit review/unlock/relock is the recovery contract. Relock makes a new promise and retains old fault history; it does not restore old content or prove the old promise held.
- [ ] **S06-AC10** Tamper isolated fixtures across text/address/Workspace/kind/member-extra-or-missing/pin/trigger cases and verify exact current/latched faults, nonduplicated events, startup repair order and orphaned-root recovery; update storage/GUI docs.

**Verification seam:** Real startup/status/GUI list+unlock flows plus deliberate isolated SQLite tampering and catalog comparisons; no production archive tampering.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)
- [S04 / #127](https://github.com/liudesheng9/zet/issues/127)

## S07 / #130 — Install lock protection safely and preserve it across Card migrations

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Make first upgrade and future migration boundaries preserve the same owner promise, with consistent backup, schema-version validation, stable row identities and explicitly audited reseals.

**Type:** AFK. **Slice:** S07. **User stories:** US033, US034, US036, US037, US043.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R047 (§2)** Migration and reseal — Additive tables now; explicit future reseal procedure — 14

- **R050 (§3)** `the crate-root storage and CLI module`: `open_db`, `initialize_database` — `open_db` opens SQLite directly. Bootstrap also opens a connection directly and runs ordered schema upgrades. — Register SQL functions on both paths, before a Card write can invoke a trigger.

- **R051 (§3)** `the crate-root storage and CLI module`: `ensure_open_model_parts`, `ensure_hub_cards`, `ensure_finding_cards` — Rebuilds copy explicit Card columns, including `row_id`, then drop and rename the table. — Keep row identities; install triggers after the final rebuild and detect unexpected trigger loss.

- **R301 (§14.1)** The feature adds tables/indexes/triggers; it does not rebuild `cards` or change its accepted column generations. Run prior Card/Markdown/Link migrations to completion, with SQL functions registered, then install the new lock schema under the edit lock. No old archive begins with an active Lock.

- **R302 (§14.1)** Use one consistent SQLite backup before first schema installation, following the repository's existing migration backup pattern. Do not copy a live WAL database file with ordinary file copy. Install the version marker only in the successful transaction. A failed installation leaves the prior archive usable and retryable, with its backup retained.

- **R303 (§14.1)** Subsequent startup validates the existing schema and protection catalog before performing repairs or migrations. Register functions on migration connections before executing any statement which can activate a trigger. Avoid calling a migration helper that acquires the edit lock from inside another held edit-lock scope; pass its existing connection/transaction to the narrow helper.

- **R304 (§14.2)** A Card-table rebuild must preserve every `row_id` and all lock tables/events. Keep lock-table references off `cards`, copy Card rows explicitly as current rebuilds already do, recreate the six triggers after the final rename, and verify all active Locks before writable startup. No active Lock can be treated as an excuse to omit protection on a newly rebuilt table.

- **R305 (§14.2)** For a future migration which intentionally changes authored representation:

- **R306 (§14.2)** Name the migration and why the existing fingerprint changes in that migration's design.

- **R307 (§14.2)** Take a consistent backup and the existing edit lock; work in one migration transaction.

- **R308 (§14.2)** Verify old baselines before applying the transformation. Refuse a preexisting integrity fault.

- **R309 (§14.2)** Compute the new authored fingerprints. Update the member baseline immediately before its matching Card transformation within that same trusted transaction, or use the Card-rebuild boundary if that migration already rebuilds the table. No public guard-bypass flag is exposed.

- **R310 (§14.2)** Preserve addresses, row ids and Link/pin meaning. A migration that needs to change those needs a separate design; an authored reseal alone does not authorize it.

- **R311 (§14.2)** Recompute each affected range seal, append `reseal` with old/new evidence, restore/verify the protection triggers, and advance revision once before commit.

- **R312 (§14.2)** There is no reseal migration in this release. Ordinary edits cannot call this path.

- **R313 (§14.2)** An older zt binary is not a supported writer after upgrading. In particular, its connection may lack the SQL fingerprint function. Stop old Sessions/MCP clients during deployment, install the new binary, restart the service, and start new clients. Keep the backup for a full rollback rather than recommending an older executable against an upgraded active-Lock archive.

- **R375 (§18.1)** T21 — Trigger lost before startup or through an authorized table rebuild — Unexpected loss recorded before repair; rebuild preserves row ids and reinstalls canonical triggers

## Acceptance criteria

- [ ] **S07-AC01** First installation is additive: finish prior Card/Markdown/Link migrations, register SQL wrappers, back up consistently through SQLite, and install all lock tables/indexes/triggers with tree_lock_schema_version 1 in a guarded transaction. Do not rebuild cards or change accepted Card column generations merely for this feature.
- [ ] **S07-AC02** Validate existing lock table/index/trigger generations rather than equating IF NOT EXISTS with compatibility. An unsupported version refuses writes without repair; a failed installation leaves prior state retryable and retains its consistent backup.
- [ ] **S07-AC03** Audit every direct writable/bootstrap/migration connection for function registration before any trigger-bearing Card write, without recursive writer-lock acquisition or migrations hidden in connection opening.
- [ ] **S07-AC04** On later startup inspect protection before repair or Card migration can erase evidence. Keep foreign keys off cards so rebuilds cannot cascade away retained Lock evidence; preserve all lock tables/events and explicit Card row_id values.
- [ ] **S07-AC05** Authorized Card rebuilds recreate the six canonical triggers after the final table rename and verify active Locks before writable startup. A missing backstop cannot be ignored because a rebuild just occurred.
- [ ] **S07-AC06** Document and test the trusted future authored-representation reseal contract: named migration, consistent backup, healthy old baseline, same transaction, paired candidate baseline/Card transformation, old/new range seal and changed-member event evidence, canonical protection and one revision boundary.
- [ ] **S07-AC07** Reseal never authorizes address/row/Link/pin-meaning changes or an ordinary edit. Add no public reseal command or guard-disable flag; this release performs no authored reseal migration.
- [ ] **S07-AC08** Older binaries/MCP clients are unsupported upgraded-archive writers and may lack SQL functions. Deployment closes old Sessions/clients, restarts the new service and retains a full backup for rollback instead of using an old executable against active Locks.
- [ ] **S07-AC09** Test first install, retry/abort, unsupported generation, trigger-loss repair order and a representative stable-row Card rebuild in isolated databases; verify backup integrity, fingerprints, event history and second-start idempotence; update storage docs.

**Verification seam:** Existing startup/schema/backup integration fixtures and a representative Card-rebuild fixture, observed through actual service/read/status behavior.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)
- [S06 / #129](https://github.com/liudesheng9/zet/issues/129)

## S08 / #131 — Show Lock marks and reliable integrity diagnostics in Session and status

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Give terminal users the same state and useful refusal guidance as GUI users without adding terminal Lock controls.

**Type:** AFK. **Slice:** S08. **User stories:** US020, US021.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R034 (§2)** D4: GUI authority — GUI routes only; normal GUI token; no MCP, shell, or Session lock control — 6, 10

- **R044 (§2)** Session and shell visibility — Marks, refusal hints, status counts; no lock command — 11

- **R231 (§11)** The Session view's title row appends `[locked]` and/or `[pinned]`. Line mode and the TUI render the same facts. Marks describe the held Card; navigating outside the tree removes the locked mark. There is no `lock`/`unlock` Session or shell command.

- **R232 (§11)** Precheck a locked destination before launching an external editor or opening a Session edit mode. Metadata commands `ty`, `lv`, `mf`, Literature metadata editing, `mv`, `ins`, `n`, `b`, and `del` still rely on the transaction guard for correctness. A Session's already-open editor can outlive a read view, so the write-time check remains mandatory.

- **R233 (§11)** `zt status` adds:

- **R234 (§11)** Exact user-facing contract: locks: 3 trees, 412 cards, 57 pinned targets; integrity ok.

- **R235 (§11)** Counts are active Locks, member snapshot rows, and distinct pinned target rows. They are not pin pair counts and do not include ended Locks. On a fault, print the faulted roots and a concise diagnostic instead of `integrity ok`. Database/read errors are reported as unavailable/error, never converted to a healthy zero-Lock result.

- **R236 (§11)** `zt status --check-locks` performs the full scan described in §13, prints its details, and exits 1 on a current or latched active-Lock fault. Ordinary status may run that same scan; do not add a persisted check-result table just to optimize a rarely invoked shell command. When the archive is not configured, preserve today's down/unset output. A scan does not start a lock operation.

## Acceptance criteria

- [ ] **S08-AC01** Both TUI and line-mode title rows mark the held Card with locked and/or pinned and clear the mark on navigation to a different state; ordinary navigation stays available.
- [ ] **S08-AC02** Precheck locked targets/destinations before opening external or Session editors, but retain transaction-time checks for an editor/read view opened before a Lock. Cover n, b, ins, mv, del, authored/Literature edits and ty/lv/mf setters.
- [ ] **S08-AC03** Owner refusals name the Locked tree and tell the user to unlock through the GUI; a pin explains the locked source. No shell/Session lock or unlock command appears.
- [ ] **S08-AC04** Status reports active tree count, member-snapshot count and DISTINCT target-row count, excluding historical Locks and pair duplicates; print integrity ok only on a healthy comparison.
- [ ] **S08-AC05** zt status --check-locks performs the full comparison and exits 1 on current/latched faults. Database or evidence-recording failure reports unavailable/error rather than a healthy zero-Lock result; preserve unconfigured down/unset behavior.
- [ ] **S08-AC06** Do not add a persistent check-result table or new polling loop just to optimize status. Test real PTY/line mode, pre-editor refusal, held-editor race and exit/output behavior in isolated archives; update Session and CLI docs.

**Verification seam:** Actual shell status exit codes and output, line-mode Session protocol and existing PTY fixtures with real service writes.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)
- [S03 / #126](https://github.com/liudesheng9/zet/issues/126)
- [S06 / #129](https://github.com/liudesheng9/zet/issues/129)

## S09 / #132 — Preserve structured MCP refusals and safe identified receipt replay

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Make every MCP writer report the same typed Lock policy and preserve the existing transaction/receipt guarantees when a tree is locked after earlier work.

**Type:** AFK. **Slice:** S09. **User stories:** US022, US023, US024.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R052 (§3)** `the Card-format module`: `authored_fingerprint` — Hashes parsed title/body, BibTeX and type fields, excluding Reverse links. — Share this algorithm. Do not use the separately defined batch receipt fingerprint as the seal.

- **R057 (§3)** `the identified-batch module` — Identified writes use caller-owned transactions, receipts and rollback previews. The loop currently returns on its first error. — Preserve real-apply atomicity; add bounded aggregate lock reporting only to preview.

- **R182 (§8.1)** Retain the proposal's object and reason vocabulary:

- **R183 (§8.1)** A structured Lock refusal exposes error card_locked, message, blocking tree root address, affected cards, exact reason and actionable hint. Pin refusals name the locked source; transport wrapping and isError remain unchanged.

- **R184 (§8.1)** Reasons are `edit`, `delete`, `move`, `create-inside`, `move-inside`, `pinned-move`, and `pinned-delete`. The canonical `tree` is the blocking Lock's root address. For pin errors also add `sources` with the locked source addresses; an optional `locks` array can explain multiple blocking Locks. Sort roots and addresses deterministically and deduplicate them.

- **R185 (§8.1)** Keep the existing MCP JSON-RPC/tool error envelope and `isError` behavior. Add the typed `tree_lock::Refusal` mapping beside `read::Limit` and `batch::Conflict`; never recover structured data by parsing `Display` text. GUI errors retain the same fields. Shell/Session render `message` and append `unlock the tree in the GUI first` where an owner action is needed.

- **R186 (§8.1)** `plan_changed`, `already_locked`, `not_locked`, invalid arguments and integrity faults remain distinct outcomes. An `integrity_fault` is not permission to refresh the baseline automatically.

- **R187 (§8.2)** `apply_batch` keeps its current item count, authored-byte bounds, local-reference rules, operation keys and transaction boundary. A new refused item rolls back all previously unseen writes and receipts in that batch, including Topic allocation and derived updates. Previously committed receipts remain unchanged.

- **R188 (§8.2)** An identical existing receipt may replay after the owner locks its result: replay performs no new Card mutation. Do not re-execute the old mutation to produce its response. Preserve existing key conflict and preview freshness checks. A new operation key requesting an edit of that locked Card is refused even if its proposed text equals the current text.

- **R189 (§8.2)** Card annotations in fresh read/preview projections come from today's lock state. Do not rewrite a stored historical receipt merely to add a current lock flag.

- **R368 (§18.1)** T14 — Historical receipt replays after Lock; new key edits same Card — Replay performs no Card write; new edit refused

- **R369 (§18.1)** T15 — Batch has earlier valid writes then locked item — Entire unseen batch/receipts/allocation/index/revision rolled back

## Acceptance criteria

- [ ] **S09-AC01** Keep the existing JSON-RPC/tool-result envelope and isError behavior, mapping a typed Refusal beside existing conflict/read-limit types without parsing English.
- [ ] **S09-AC02** Expose error card_locked, message, blocking root-address tree, deduplicated cards, reason and hint; reasons are edit, delete, move, create-inside, move-inside, pinned-move and pinned-delete. Pin errors also identify locked sources and deterministic multiple-blocker details.
- [ ] **S09-AC03** Workspace denials still apply first for agent human-Workspace writes. A separately allowed operation receives the Lock refusal; plan_changed, already_locked, not_locked, invalid arguments and integrity_fault remain distinct.
- [ ] **S09-AC04** All single tools and all transaction/owned-paragraph paths use the shared gate, including metadata/structural operations and no-op edit requests. There is no MCP Lock tool, alternative Access or GUI-control dispatch.
- [ ] **S09-AC05** An identical committed receipt can replay after its result becomes locked without repeating a Card write; a new key requesting that edit refuses. Preserve existing key-conflict and freshness rules and do not rewrite historical receipt JSON to add today's annotation.
- [ ] **S09-AC06** An unseen refused batch rolls back all unseen Cards, receipts, Topic allocation, Links, generated updates and revision while keeping previously committed receipts. Preserve current count, authored-byte, local-reference and transaction bounds.
- [ ] **S09-AC07** Public single/batch/receipt-replay fixtures compare complete before/after state and structured results, not English substrings alone; update MCP/Card-operations docs with no new lock authority.

**Verification seam:** Real stdio MCP process calls and identified batch/receipt fixtures using isolated SQLite state snapshots.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)
- [S03 / #126](https://github.com/liudesheng9/zet/issues/126)

## S10 / #133 — Aggregate Lock refusals in rollback-only batch previews

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Extend the existing real preview loop so planners see every independently checkable Lock refusal without inventing addresses or persisting a partially simulated plan.

**Type:** AFK. **Slice:** S10. **User stories:** US025.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R028 (§1.2)** Batch preview failures — Continue after a lock refusal through independent items; label dependent items as blocked — Satisfies useful aggregate reporting without inventing addresses for refused creations.

- **R190 (§8.3)** Extend the existing preview loop rather than adding a second planner. Only preview uses per-item SQLite savepoints and an item status list:

- **R191 (§8.3)** Validate the request shape and whole-request bounds before simulation, as today.

- **R192 (§8.3)** Resolve earlier successful references and begin a savepoint for the item.

- **R193 (§8.3)** Run the normal receipt and writer path with the normal Lock check.

- **R194 (§8.3)** On success, release the savepoint into the outer temporary transaction. Publish the item's reference binding and changed-source facts only after success.

- **R195 (§8.3)** On `card_locked`, roll back that savepoint, record the refusal with item index, operation key and client reference, and continue to the next independent item.

- **R196 (§8.3)** If an item needs a reference produced by a refused or blocked item, record `blocked_by_refusal`. Do not fabricate an address or claim the dependent write was validated.

- **R197 (§8.3)** Preserve ordinary non-Lock validation errors and their current failure contract. Do not turn preview into a general error-recovery engine.

- **R198 (§8.3)** Roll back the outer transaction before serializing any response, including an oversized or unsuccessful response.

- **R199 (§8.3)** An unsuccessful preview has `preview: true`, `applied: false`, `complete: false`, the unchanged base `archive_revision`, ordered lock refusals, and blocked-item identities. Successfully simulated items may be identified as such, but they are not committed receipts or a successful whole-plan projection. Report every independently resolvable locked item reached by the preview. Explicitly report any items that could not be checked; never claim that a dependency-blocked plan was fully scanned.

- **R200 (§8.3)** The existing `preview_items` sequence uses the same logic across simulated chunk boundaries. Keep its 1,024-item/8-MiB bound, real chunk rules, and encoded-response limit. A real batch remains fail-fast and atomic; aggregation does not authorize partial application.

- **R370 (§18.1)** T16 — Preview includes several independent locked writes and a dependency on a refused create — All resolvable refusals reported; dependent item marked blocked; outer rollback proven

## Acceptance criteria

- [ ] **S10-AC01** Validate request shape and existing whole-request bounds before simulation; use the same receipt/writer/parser/guard path, not a separate planner.
- [ ] **S10-AC02** In preview only, use one savepoint per item. Publish reference bindings and changed-source facts only after success; a Lock refusal rolls back that item and records its index, operation key and client reference while independent later items continue.
- [ ] **S10-AC03** A reference depending on a refused/blocked creation produces blocked_by_refusal with the affected item identity; do not fabricate a target address or claim it was checked. Preserve ordinary non-Lock validation failures rather than building a general recovery engine.
- [ ] **S10-AC04** An unsuccessful preview returns preview true, applied false, complete false, unchanged base archive_revision, ordered refusals and explicitly blocked/unchecked items. Successful simulations are not committed receipts or a valid whole-plan projection.
- [ ] **S10-AC05** Always roll back the outer transaction before response serialization, including encoded oversize/error responses; Cards, receipts, allocation, Link diagnostics/index and revision remain unchanged.
- [ ] **S10-AC06** preview_items uses the same aggregation across simulated chunk boundaries while keeping its 1,024-item/8-MiB bounds, actual 64-item/1-MiB chunks and existing encoded result limit. Real application remains fail-fast and atomic.
- [ ] **S10-AC07** Test multiple independent failures, dependent chains, a successful prefix, existing receipt replay, count/byte chunk boundaries, ordinary validation failure and oversized reply; verify database state independently; update batch/MCP docs.

**Verification seam:** Existing public apply_batch preview and preview_items suites backed by real service/MCP and database rollback oracles.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S09 / #132](https://github.com/liudesheng9/zet/issues/132)

## S11 / #134 — Empty a Topic or Literature tree while keeping its root

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Add shell keep-root and Session del keep-root through the existing delete plan, preserving the root and every Link to it while removing its Regular Cards.

**Type:** AFK. **Slice:** S11. **User stories:** US026, US027, US028, US029, US031.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R030 (§1.2)** Emptying an already empty locked tree — Refuse before the prompt — A Lock forbids the requested tree operation even when its delete set is empty.

- **R062 (§3)** `the crate-root storage and CLI module`: `DeletePlan`, `apply_delete_plan` — A delete may re-point inbound Links, delete Cards, compact Side successors and update Spreads in one transaction. — Check every effect, not only the named Card. Keep this path for tree-emptying.

- **R246 (§12.2)** Add a small scope field to the current plan:

- **R247 (§12.2)** DeleteScope distinguishes Subtree, whole Tree, KeepRoot with retained root and All; retain existing delete-set, confirmation, inbound/repoint and compaction data and add only scope/root/reset information.

- **R248 (§12.2)** Keep `delete_set`, `confirmation`, `inbound`, `into` and `side_compaction`. Add only the scope/root information required for validation, display and the Topic-number reset. The all-archive plan has every Card address, no staying inbound sources, no compaction, no `into`, and confirmation `delete all`. The keep-root plan has every member except the root, no compaction, and confirmation equal to the root address.

- **R249 (§12.2)** Use one internal transaction apply function for all scopes. An existing wrapper may open and commit the transaction; clients with a caller-owned transaction use the internal function. This is a narrow extraction of `apply_delete_plan`, not a move of all storage code out of `main module`.

- **R250 (§12.2)** Before application, check scope restrictions, locked members, pin targets, re-pointing and actual compaction. The `All` scope checks active Locks directly, not only intersection with existing Cards. That protects an orphaned or damaged Locked tree as well.

- **R251 (§12.3)** The verification plan identifies the root, Workspace, number of Regular Cards to remove, `root kept`, and Links from staying Cards that will break. Print the first 20 breaking-Link pairs and an exact remaining count, as the proposal specifies. Keep the complete inbound set internally for checks and Link regeneration.

- **R252 (§12.3)** After confirmation, one transaction:

- **R253 (§12.3)** Revalidate the scope and lock effects.

- **R254 (§12.3)** Delete only the selected Regular Cards.

- **R255 (§12.3)** Call `spread::follow` with the deleted set, so deleted positions disappear and root positions remain.

- **R256 (§12.3)** Rebuild the derived Link index and refresh generated Reverse links.

- **R257 (§12.3)** Advance the revision once when archive state changed and commit.

- **R258 (§12.3)** The root keeps its authored identity, address, Workspace, BibTeX and kind. Its generated Reverse links may change because deleted Cards no longer link it. Outgoing root Links to deleted Regular Cards become Broken links and belong in the verification. Links from other Cards to the root remain valid.

- **R259 (§12.3)** Do not change `next_topic_id` or `applied_ops`. No per-tree next-Card counter is needed: the existing successor rule yields `3/1` or `Smith2024/1` after the Regular Cards are gone.

- **R260 (§12.3)** Success output:

- **R261 (§12.3)** Exact user-facing contract: deleted: 214; root kept: grossman2002integration.

- **R262 (§12.3)** For an already empty unlocked tree, still show the plan and require the root confirmation. Return `deleted: 0; root kept: <root>` without a revision or derived rebuild. An empty locked tree is refused before that prompt.

- **R263 (§12.3)** This operation is allowed while a Session is open. Retain `recover_missing_pointer`: a Pointer to a deleted Card falls back to an existing ancestor, eventually the retained root. Add Session syntax `del keep-root` only on a Tree root; ordinary `del` keeps its existing behavior. `del keep-root` on a Regular Card gives the root-kind error. No `--all` Session form is added.

- **R382 (§18.2)** D03 — Empty Topic/Literature tree with keep-root — Root fingerprint/BibTeX and inbound Links retained; next successor is `/1`; global Topic allocation unchanged

- **R383 (§18.2)** D04 — Root body links deleted child; external Card links root and child — Root Link stays valid from outside; child Links reported as breaking; generated sections/index exact

- **R384 (§18.2)** D05 — Keep-root with active Session and Pointer on a deleted child — Allowed; Pointer recovers to surviving root/ancestor

- **R386 (§18.2)** D07 — Locked populated/empty tree; pinned root only; pinned Regular child — Locked requests refused even empty; root-only pin permits keep-root; child pin refuses it

## Acceptance criteria

- [ ] **S11-AC01** zt del --at <root> --keep-root and Session del keep-root require an actual Tree root and its exact address as confirmation. Wrong kind, malformed selectors, cancellation and EOF change nothing; precheck Lock/pin effects before prompting.
- [ ] **S11-AC02** Extend DeletePlan with narrow scope/root facts while retaining delete_set, inbound, into and actual Side compaction. Keep-root selects all members except root and uses no compaction; apply through the existing common transaction function.
- [ ] **S11-AC03** Verification identifies root, Workspace, Regular-Card removal count, root kept and inbound Links from every staying Card including the root; show the first 20 breaking pairs plus an exact remaining count, retaining the full set internally.
- [ ] **S11-AC04** One transaction rechecks scope/pins, deletes selected Regular Cards, drops their Spread positions, rebuilds Links/Reverse sections and advances revision once only on change. Failure rolls all affected state back.
- [ ] **S11-AC05** Preserve root address, authored fingerprint, Workspace, kind and BibTeX; its generated Reverse section may change. External root Links remain valid and root/other Links to deleted children become explicitly reported Broken links.
- [ ] **S11-AC06** Keep next_topic_id and applied_ops unchanged. The next Regular Card is root/1 under existing successor rules. A fresh digest rerun uses a fresh operation prefix rather than replaying old keys.
- [ ] **S11-AC07** Allow an active Session; recover a deleted Pointer to a surviving ancestor/root. A locked tree refuses even if already empty; an unlocked empty tree still confirms and reports zero without revision/rebuild.
- [ ] **S11-AC08** A pin on the retained root alone permits emptying, while a pinned deleted child or another forbidden effect refuses. Do not freeze the entire target tree because one retained Card is pinned.
- [ ] **S11-AC09** Print deleted count and root kept exactly as specified. Test Topic/Literature, root-body Links, external Links, Spreads, pointer recovery, /1 reuse, cancellation/rollback and pin cases; update Session, CLI and delete design/help docs.
- [ ] **S11-AC10** Do not add GUI/MCP keep_root or delete-all arguments in this release; existing remote delete forms use the same Lock-aware plan unchanged.

**Verification seam:** Real shell pipe confirmations, Session line/PTY flows, MCP read/Link integrity and direct retained-root/Spread/allocation comparisons.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)
- [S02 / #125](https://github.com/liudesheng9/zet/issues/125)

## S12 / #135 — Unify archive deletion under zt del with Session and Lock exclusion

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Finish the joined delete command, including all-archive reset, exact confirmations and a one-release non-destructive clear notice, while preserving existing cleanup boundaries.

**Type:** AFK. **Slice:** S12. **User stories:** US028, US029, US030, US031.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R036 (§2)** D6: delete consolidation and protection — Four `zt del` forms; no active-Lock override; one-release `zt clear` notice — 12

- **R063 (§3)** `the crate-root storage and CLI module`: `cmd_clear` — Deletes all Cards, rebuilds Links and resets `next_topic_id`; leaves other archive metadata and files. — Fold the behavior into `zt del --all`, adding the Lock refusal and common delete application.

- **R065 (§3)** `the Session module` — Session registration has its own existing `zt.sessions.lock`. — Reuse it to serialize the no-open-Session check for `--all`; see §12.4.

- **R237 (§12.1)** Exact user-facing contract: zt del --at <card>; zt del --at <root>; zt del --at <root> --keep-root; zt del --all.

- **R238 (§12.1)** Parse into one small command enum, such as `At { address, keep_root } | All`. Reject duplicate selectors/flags and unknown arguments. Exactly one of `--at` and `--all` is required. `--keep-root` requires `--at` and an actual Tree root. Check syntax and target kind before any prompt; resolving target kind still requires the service/database.

- **R239 (§12.1)** Use the proposal's usage and Regular-Card error text. The four confirmation words remain:

- **R240 (§12.1)** Regular Card subtree — `delete`

- **R241 (§12.1)** Whole tree — Root address

- **R242 (§12.1)** Keep root — Root address

- **R243 (§12.1)** Whole archive — `delete all`

- **R244 (§12.1)** Read exactly one stdin line and remove only its line terminator. A mismatched word or EOF cancels with no SQLite change. The command remains usable through a pipe. A cancellation is not a Lock refusal: lock checks happen before the prompt.

- **R245 (§12.1)** In 0.22.0, `zt clear` prints ``zt clear is now `zt del --all` `` and exits 1 without requiring the service, prompting, or deleting anything. Remove it from help immediately. In the next release, remove the notice handler too, leaving an unknown command. Update shell documentation and tests around the new command; do not keep a working destructive alias.

- **R264 (§12.4)** Keep today's `clear` effects on Cards and allocation, under the new command:

- **R265 (§12.4)** Delete all `cards` and derived `links` rows.

- **R266 (§12.4)** Set `metadata.next_topic_id` to `0`.

- **R267 (§12.4)** Keep config, dump files, artifacts, other metadata and existing `applied_ops` receipts.

- **R268 (§12.4)** Keep ended Locks and their history. An active Lock always refuses the operation.

- **R269 (§12.4)** For `All`, preserve today's `clear` handling of saved Spreads: their saved metadata remains. Tree and subtree deletes, including keep-root, continue to use `spread::follow`. This difference is intentional compatibility with the proposal's “same effect as today's clear”; do not silently broaden the feature into Spread cleanup or artifact garbage collection.

- **R270 (§12.4)** The no-open-Session rule must hold through the write, not just before the prompt. Use the existing Session-registration lock (`zt.sessions.lock`): acquire the edit lock, then the Session-registration guard; prune/check active Session pids while holding that guard, and keep it through confirmation and commit. A newly starting Session waits for registration until the deletion ends. Inspect the existing registration/count helpers and extract a crate-private guarded check rather than calling `session_count` recursively while already holding its lock.

- **R271 (§12.4)** Use this lock order consistently: **edit lock, then Session-registration lock**. Session registration takes only its registration lock and releases it before running commands. Do not introduce any path that holds the registration lock while waiting for the edit lock. The command can hold these guards while awaiting stdin, as today's clear already holds the edit lock; EOF or cancellation releases both.

- **R272 (§12.4)** The all-archive plan checks active Locks before printing the confirmation prompt and again in the transaction. On refusal, list locked root addresses and their count, with the GUI unlock hint. There is no `--force` option. A failed SQL/derived update rolls back Cards, Links and allocation together.

- **R273 (§12.4)** Print:

- **R274 (§12.4)** Exact user-facing contract: deleted: 1872 cards; next topic id reset to 0.

- **R275 (§12.4)** If the Card table is already empty but allocation was nonzero, resetting allocation is still a state change and advances the revision once. If both were already empty/zero, the confirmed operation succeeds without a revision. Failed/cancelled operations advance nothing.

- **R276 (§12.5)** Add keep-root to shell and Session only in this release. Existing GUI and MCP `delete_card` keep their current arguments and confirmations, while using the updated Lock-aware plan. They do not gain `keep_root` or delete-all tools yet. The shared plan structure makes a later explicitly requested interface addition possible without another deletion algorithm.

- **R277 (§12.5)** Old receipts deliberately survive tree emptying and delete-all. A digest rerun must use a fresh run/operation prefix; replaying old keys is not a way to recreate deleted Cards. Keep this in the rerun instructions rather than changing the existing receipt protocol in this feature.

- **R380 (§18.2)** D01 — Both/no selectors, duplicate flags, unknown flags, keep-root with `--all` or a Regular Card — Refused before prompt; no side effect

- **R381 (§18.2)** D02 — Each of the four selections; wrong word, EOF and exact confirmation — Correct word per scope; cancel changes no state

- **R385 (§18.2)** D06 — All-archive with existing Session or Session attempting to register during confirmation — Existing Session refuses deletion; new registration cannot interleave the protected check/commit

- **R387 (§18.2)** D08 — All-archive with active Lock whose root/member is damaged or missing — Refused based on active Lock records before prompt

- **R388 (§18.2)** D09 — Failure during delete, Link rebuild or allocation reset — Cards, Links, Spreads and allocation roll back together

- **R389 (§18.2)** D10 — Topic number after subtree/tree/keep-root, then after all-archive — Allocation advances as before except explicit all-archive reset to `0/0`

- **R390 (§18.2)** D11 — Keep-root vs all-archive metadata/files — Keep-root drops deleted Spread positions; all retains old clear metadata behavior; receipts/artifacts/dumps/config survive

- **R391 (§18.2)** D12 — Legacy `zt clear`, shell/Session help, GUI/MCP schemas — 0.22 notice and exit 1; only supported forms advertised; no new remote delete-all authority

## Acceptance criteria

- [ ] **S12-AC01** Support exactly one selector, at or all; reject both/neither, duplicates, unknown arguments and keep-root with all before any prompt. Regular subtree confirms delete, whole/kept tree confirms root address, and all confirms delete all.
- [ ] **S12-AC02** All is shell-only and uses the common DeletePlan transaction path: select all Cards, no staying inbound sources/compaction/into, delete cards/derived links and reset next_topic_id to zero atomically.
- [ ] **S12-AC03** Refuse all whenever any active Lock record exists, including a missing/damaged root or empty Card table. Report root addresses/count and GUI unlock hint before prompting and revalidate in the transaction; add no force option.
- [ ] **S12-AC04** Refuse existing Sessions and serialize Session registration through the complete confirmation/commit interval using the existing registration mutex. Always acquire edit mutex before registration mutex; registration never waits for edit while holding its guard, and avoid recursively taking the registration lock.
- [ ] **S12-AC05** A Session trying to start during all waits to register until deletion ends. Wrong input, EOF, failure or process exit releases guards without a partial delete. Retain ordinary at/keep-root availability with open Sessions.
- [ ] **S12-AC06** Preserve config, dump files, artifacts, other metadata, applied_ops and ended Lock/history records. All keeps today's clear behavior for saved Spread metadata; subtree/tree/keep-root still follow deleted positions. Do not add garbage collection.
- [ ] **S12-AC07** Topic numbers are reset/reused only by all; empty Cards with nonzero allocation still count as a revision-changing reset, while empty/zero confirmed state is a no-op. Preserve exact output deleted count and next topic id reset to 0.
- [ ] **S12-AC08** In 0.22.0 clear prints its migration notice and exits 1 without service start, prompt or deletion; remove it from help and retain no destructive alias. Remove the notice handler in the following release as a documented follow-up.
- [ ] **S12-AC09** Keep all unavailable in Session and remote tool schemas. Test exact parsing/words, all-current-delete regressions, concurrent registration, active/orphaned Locks, reset/no-op, preservation and rollback; update help/CLI/Session and dump/delete design docs.

**Verification seam:** Real shell process and stdin confirmation tests, concurrent Session registration fixtures, complete SQLite/file preservation and allocation comparisons.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S11 / #134](https://github.com/liudesheng9/zet/issues/134)
- [S08 / #131](https://github.com/liudesheng9/zet/issues/131)

## S13 / #136 — Include portable Lock history and integrity evidence in every dump

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Extend the existing compressed dump so its Card snapshot and lock/history evidence describe the same archive view and remain inspectable after old roots disappear.

**Type:** AFK. **Slice:** S13. **User stories:** US032.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R042 (§2)** Dump — Add versioned `locks.json` under the existing dump transaction/view — 14

- **R064 (§3)** `the dump module` — Writes named payload entries and checks compression output. — Add an explicit `locks.json` entry; merely writing a file into staging would not include it in every archive format.

- **R314 (§14.3)** Extend the existing dump input with an explicit lock-export value read while the dump holds the edit lock, from the same SQLite view as its Cards and `next_topic_id`. The dump module remains a serializer/compressor; it does not open another connection to discover Locks later.

- **R315 (§14.3)** Add `locks.json` to staging **and** the explicit compressed-entry list, immediately after `archive.json`. Keep existing `mapping.json`, `archive.json`, Card text, BibTeX, artifact and compression behavior unchanged. Include `locks.json` even when there are no Locks, with empty arrays and `schema_version: 1`.

- **R316 (§14.3)** The file contains:

- **R317 (§14.3)** Export schema version, archive identity, and captured revision.

- **R318 (§14.3)** All active and ended `locks` records.

- **R319 (§14.3)** Active member snapshots with addresses, row identities and authored fingerprints.

- **R320 (§14.3)** Active pins with captured source/target addresses and row identities.

- **R321 (§14.3)** All events in event-id order, including the initial snapshots of ended Locks.

- **R322 (§14.3)** Integrity facts available from a scan of the same captured state.

- **R323 (§14.3)** Addresses and Lock ids are the portable interpretation. Row ids retain audit identity inside that captured database; a future restore must resolve them explicitly rather than assuming they are new database row ids. Order objects/lists deterministically. An integrity fault is exported as evidence and does not prevent taking a diagnostic dump. Failure to serialize/read the required lock data fails the dump through its existing cleanup path.

- **R324 (§14.3)** The dump contains enough to inspect lock history, but this feature does not implement restoration or promise that a future restore imports Locks automatically.

- **R378 (§18.1)** T24 — Dump with active/ended/faulted/no Locks — Required `locks.json` included and ordered; snapshots/history preserved; no source mutation

## Acceptance criteria

- [ ] **S13-AC01** Capture Lock export, Cards, archive identity/revision and next Topic id from the same guarded SQLite view. Pass explicit export data to the serializer; it must not open a later independent connection.
- [ ] **S13-AC02** Write version-1 locks.json even with no Locks, with all active/ended Lock records, active member snapshots, source/target pin addresses and row identities, all ordered events and captured integrity facts.
- [ ] **S13-AC03** Ended initial lock events retain their baseline snapshots after unlock removes live rows. Addresses/Lock ids give portable meaning; row numbers are audit evidence and no future restore may blindly reuse them.
- [ ] **S13-AC04** Add locks.json to both staging and every explicit compression entry list immediately after archive.json. Keep mapping/manifest/Card/BibTeX/artifact ordering and existing compressor detection/signature/cleanup behavior.
- [ ] **S13-AC05** A fault can be dumped diagnostically with the fault evidence; required export serialization/read failure fails through existing cleanup. A dump is not a new restore command or promise of automatic future Lock import.
- [ ] **S13-AC06** Extract every supported fixture format and verify active/ended/faulted/empty exports, deterministic order, matching revision/Card identities, historical missing roots and no source mutation; update dump/storage docs.

**Verification seam:** Actual zt dp compression and extraction fixtures with known Card/Lock snapshots and existing failure cleanup oracles.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S01 / #124](https://github.com/liudesheng9/zet/issues/124)
- [S04 / #127](https://github.com/liudesheng9/zet/issues/127)
- [S06 / #129](https://github.com/liudesheng9/zet/issues/129)

## S14 / #137 — Stop digest write modes before dispatch to locked destinations

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Make current digest preparation and compiler entry points identify actual write targets before allocating jobs, while preserving read-only use of locked content and server authority during races.

**Type:** AFK. **Slice:** S14. **User stories:** US038, US041.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R327 (§15.2)** Read-only audit, model report, checker, export — Read normally — No write to precheck

- **R328 (§15.2)** Digest `cards`, `models`, math, events, findings, figures or other rewriting mode — Read normally — Stop before allocating jobs or writing Cards

- **R329 (§15.2)** Full digest into an existing Literature tree — Read normally — Stop before the first skeleton/metadata write

- **R333 (§15.2)** For book modes that write the Literature tree, preparation returns:

- **R334 (§15.2)** Exact user-facing contract: the Literature tree <key> is locked by the owner.

## Acceptance criteria

- [ ] **S14-AC01** For full or rewriting cards/models/math/events/findings/figures/skeleton/metadata modes targeting an existing locked Literature tree, stop before the first write and before job allocation/dispatch with the specified Literature-tree locked-by-owner message.
- [ ] **S14-AC02** Determine restrictions from the selected mode's actual destinations; audits/checks/exports/model reports and locked sources remain readable. Do not reject all agent work merely because some unrelated tree is locked.
- [ ] **S14-AC03** Cover current shared preparation/client helpers and relevant native/PowerShell entry points, preserving job/ledger/operation ownership and structured card_locked data across helper boundaries.
- [ ] **S14-AC04** Preparation is advisory: if a tree locks mid-run, stop the affected write stage, retain staged drafts/errors and earlier committed receipts, and distinguish one rolled-back batch from earlier committed batches.
- [ ] **S14-AC05** Never repeatedly retry card_locked as a provider/transient error, change Access, invoke GUI Lock routes, or relabel a stopped run as complete. A fresh rerun after keep-root uses fresh operation keys.
- [ ] **S14-AC06** Use simulated modes/jobs and real isolated MCP preflight/mid-run races to prove no dispatch for known locked destinations and valid locked-source reads; update digest/compiler workflow docs.

**Verification seam:** Existing digest preparation and fake-job harnesses with actual isolated MCP replies and mid-run owner Lock changes; no real book/provider run.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S03 / #126](https://github.com/liudesheng9/zet/issues/126)
- [S09 / #132](https://github.com/liudesheng9/zet/issues/132)

## S15 / #138 — Read locked books in idea-grow and file only into unlocked Topics

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Allow the useful idea-grow case from the proposal: a finished locked Literature source remains readable, but generated agent Cards are placed only in permitted Topic trees.

**Type:** AFK. **Slice:** S15. **User stories:** US039, US041.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R066 (§3)** `the idea-grow implementation`, `the idea-grow tooling` — Idea-grow planning reads source books and destination trees; implementation is present as working changes. — Add lock state to destination selection after that work settles. Reading a locked book remains valid.

- **R330 (§15.2)** Idea-grow from a finished book — Read the locked Literature tree normally — File only into unlocked agent Topic trees

- **R335 (§15.2)** A precheck must use the intended mode's actual destinations. Do not reject idea-grow merely because its book source is locked, and do not reject all agent writing merely because some agent tree is locked. Destination planners can still read locked Topics as related context, but must exclude them from `at`/successor choices. If a caller explicitly supplied a locked destination, report the conflict rather than silently substituting a tree.

## Acceptance criteria

- [ ] **S15-AC01** Read a locked source Literature tree normally; include locked Topics as related context without treating them as candidate insertion/edit destinations.
- [ ] **S15-AC02** Filter planning/title-export/tree reads by current lock state and agent Workspace; choose unlocked Topic parents for new Cards and preserve the existing growth/Link/duplicate rules.
- [ ] **S15-AC03** An explicitly supplied locked destination returns a clear conflict instead of silently changing tree or placement. Missing/stale annotations cannot be treated as an authorization to write.
- [ ] **S15-AC04** Use server guard results at application time; a newly locked destination stops the affected stage, retains drafts/prior receipts and does not trigger bypass or repeated retry.
- [ ] **S15-AC05** Integrate with the settled existing idea-grow implementation rather than overwriting unrelated working changes or inventing another state store. Keep read-only source behavior separate from destination limits.
- [ ] **S15-AC06** Verify source-locked/destination-open, related locked Topics, all/exact destination restrictions and mid-run races in existing grow fixtures; update idea-grow and digest-tool docs.

**Verification seam:** Existing idea-grow fake-job/planning harnesses, exported title/tree projections and real isolated MCP application with locked source books.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S03 / #126](https://github.com/liudesheng9/zet/issues/126)
- [S09 / #132](https://github.com/liudesheng9/zet/issues/132)

## S16 / #139 — Keep hub, register and organize plans within Lock and pin limits

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Let navigation and organization read reviewed work while preventing their edits, merges and address changes from disturbing Locked trees or pinned references.

**Type:** AFK. **Slice:** S16. **User stories:** US040, US041.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R331 (§15.2)** Hub/register run — Include locked Cards as readable/linkable members — Edit/create hubs only at unlocked destinations; reject a move/delete of a pin

- **R332 (§15.2)** Organize/merge/compaction — Read normally — Preview full structural effects; keep blocked Cards in place or return a revised plan

- **R336 (§15.2)** Keep pin checks scoped to operations which change address/existence. A pinned, unlocked Hub Card can be edited if ordinary rules permit; pinning does not make every compiler update illegal.

## Acceptance criteria

- [ ] **S16-AC01** Include locked Cards as read-only hub/register members and authored Link targets; only actual hub/register write destinations must be unlocked.
- [ ] **S16-AC02** Guard how edit and side placements through ordinary create/edit/metadata paths. Hub draft and precheck remain read-only; Save cannot create a partial Card in a locked destination.
- [ ] **S16-AC03** Keep an unlocked pinned Hub editable when normal domain rules permit. Pins block address/existence changes, not all compiler updates or every Card in its tree.
- [ ] **S16-AC04** Organize/merge/move/insert/delete plans preview the full final mapping, into re-pointing and Side compaction effects. A pinned target or locked member keeps its position or yields a revised explicit plan; never silently drop an effect to force success.
- [ ] **S16-AC05** Preserve structured Lock refusals and distinguish blocked dependent items from fully checked plans. Mid-run Locks stop affected writes and preserve staging/earlier receipts.
- [ ] **S16-AC06** Test hub edit/side/Spread filing, register references, pinned editable Hubs, compound organizes and source/destination races in real isolated fixtures and current harnesses; update hub/GUI/compiler docs.

**Verification seam:** Existing real hub draft/Save, register and organize preview/compile fixtures with actual MCP/state checks.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S02 / #125](https://github.com/liudesheng9/zet/issues/125)
- [S03 / #126](https://github.com/liudesheng9/zet/issues/126)
- [S09 / #132](https://github.com/liudesheng9/zet/issues/132)
- [S10 / #133](https://github.com/liudesheng9/zet/issues/133)

## S17 / #140 — Publish lock-aware frozen Workflows and agent integrations

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Ship the owner policy through normal agent instructions and versioned frozen installs, so old-client server safety and new-client early stopping are clearly distinguished.

**Type:** AFK. **Slice:** S17. **User stories:** US022, US041, US042, US046.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R045 (§2)** Agent guidance — Structured errors and skill/server text, without lock-route instructions — 8, 15

- **R046 (§2)** Workflow integration — Destination prechecks, locked sources allowed, new frozen versions — 15

- **R325 (§15.1)** Add the proposed instruction to the MCP server: a Card with `lock` set may be read and linked; continue the thought in another tree. Update `the zt skill` and `the zettelkasten skill` with the same rule and the instruction that only the owner changes Locks. Do not include GUI route names or token-acquisition instructions in agent-facing material.

- **R326 (§15.1)** A `card_locked` error is a planning/destination error, not a transient provider failure. Do not retry it repeatedly, request a different Access, call the GUI routes, change the database directly, or substitute another client's write path.

- **R337 (§15.3)** Preparation reduces wasted work; the storage gate remains authoritative. If a tree becomes locked during a run, stop the affected write stage and preserve staged drafts, errors and previously committed receipts. An atomic batch refuses as a whole; earlier committed batches are not undone. Do not describe the entire Workflow as rolled back or completed successfully.

- **R338 (§15.3)** Update the current shared preparation/client helpers used by `zt-digest` and relevant PowerShell entry points. Keep error JSON intact through helper layers so the Workflow can recognize `card_locked` without matching English strings. Preserve existing job/ledger ownership and retry contracts. Do not add another workflow state store for Locks.

- **R339 (§15.3)** Book, math, idea-grow and hub Workflow installs are frozen. Change their source modules/blueprints, then assemble, simulate, verify and publish new versions through the existing installer protocol. Do not patch an installed frozen script in place or claim that an old running version picked up the new preparation check. The server gate protects old clients, but early stopping belongs to the new client versions.

- **R340 (§15.3)** After skill/server integration changes, use `the existing agent-integration installer` as required by the repository. Tests use isolated archives and fake jobs; implementing this feature does not require running a real book digest against the owner's archive.

## Acceptance criteria

- [ ] **S17-AC01** MCP instructions and the two relevant skills say locked Cards are readable/linkable sources, new thought belongs in another unlocked tree and only the owner changes Locks.
- [ ] **S17-AC02** Do not expose Lock tools/commands/routes, token acquisition or bypass recipes in agent-facing material. Preserve policy-based authority limits; a Card lock does not grant human-Workspace access or freeze external documents.
- [ ] **S17-AC03** Classify card_locked as a destination/planning error and preserve its JSON through clients. Stop affected stages without automatic unlock, changed Access, direct SQL, another writer route or transient provider retry.
- [ ] **S17-AC04** Change shared source modules/blueprints then assemble, simulate, verify and publish new frozen versions for book/math, idea-grow and hub-related Workflows through their existing protocol. Never patch installed frozen scripts in place.
- [ ] **S17-AC05** Retain staged drafts/errors, real committed receipt evidence and existing ownership/retry contracts; add no new Lock workflow store. Do not claim running old versions adopted new preparation logic, although the server gate still protects them.
- [ ] **S17-AC06** Run the existing agent-integration installer after skill/server changes, verify registration/material, restart relevant clients for new metadata and record the actual newly published versions.
- [ ] **S17-AC07** Use fake jobs and isolated archives for qualification; do not run a production book digest or change real owner Locks as a release test. Update agent-integration, native-tool and Workflow docs with code/install evidence.

**Verification seam:** Existing frozen assemble/simulate/verify harnesses, integration installer verification and isolated real-MCP refusal propagation.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S14 / #137](https://github.com/liudesheng9/zet/issues/137)
- [S15 / #138](https://github.com/liudesheng9/zet/issues/138)
- [S16 / #139](https://github.com/liudesheng9/zet/issues/139)

## S18 / #141 — Qualify Tree Lock release behavior and synchronize domain documentation

## Parent

Parent PRD: [#123 — [PRD] Freeze reviewed ZT trees and unify deletion under zt del](https://github.com/liudesheng9/zet/issues/123)

## What to build

Close the implementation release with requirement-by-requirement public evidence, synchronized domain/help/wiki contracts and isolated installed-binary qualification. This issue does not apply Locks to real owner content.

**Type:** AFK. **Slice:** S18. **User stories:** US043, US044, US045, US046.

**Source:** Tree Lock Implementation Design, SHA-256 fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a. The clauses below retain the owning contracts as prose so this issue is independently usable; they describe planned behavior, not completed work.

### Detailed design requirements

- **R341 (§16)** Let M be the number of active locked members, P the number of distinct source/target pin pairs, and E the number of stored events. Storage is O(M + P + E). The initial event also retains each baseline snapshot for history. There is no copy of complete Card bodies.

- **R342 (§16)** Lock creation hashes the selected tree and verifies its outgoing authored Links once. Ordinary member edits/deletes and pin checks use indexed row lookups. A create checks one active root. Mapping checks are proportional to the existing mapping and affected authored sources, not an extra global permission scan. Existing structural Link regeneration may still scan the archive; this feature does not redesign it.

- **R343 (§16)** A full integrity scan hashes active members and verifies their pins/catalog. It is deliberately more expensive than an ordinary read and does not run on each heartbeat. Read annotations are bulk-loaded into the existing view; no background lock worker is needed.

- **R344 (§16)** The guarantee is local and policy based. It covers writes through supported zt paths and detects specified drift at check points. It does not authenticate a deliberate local process as human, continuously watch the database, freeze external documents, or prove historical integrity after an attacker rewrites both content and baseline evidence.

- **R345 (§17)** Retain the six source delivery groups as feature coverage groups. The reviewed tracker breakdown refines them into eighteen independently verifiable AFK issues, each with its affected block documentation updated in the same code commit; tracker publication is planning rather than implementation.

- **R346 (§17)** 1. Storage and gate — Four tables, SQL wrappers/triggers, typed refusal, snapshots/pins, lock operations, integrity scan, every low-level write guard — None — Isolated fixture Locks block both Owner and Agent paths; generated refresh succeeds; failures leave no partial state

- **R347 (§17)** 2. GUI — Plan/confirm routes, note/history, band/seal, pin and fault UI, draft retention — 1 — Real API confirmation/race tests and browser checks for every disabled action/draft case

- **R348 (§17)** 3. Session, status and dump — Marks, status/check command, versioned `locks.json` — 1 — Session/CLI fixtures and extraction of every supported test dump format

- **R349 (§17)** 4. Joined delete command — Unified plan application, keep-root/all, argument rules, Session form, clear notice — 1 — All deletion forms, confirmations, pins, no-Session race and rollback tests

- **R350 (§17)** 5. MCP and agent integrations — All read projections, structured errors, batch preview aggregation, skills — 1 — Public MCP single/bulk/text/batch/preview/replay tests; integration install verification

- **R351 (§17)** 6. Workflows — Source/destination prechecks and newly frozen versions — 5 and relevant settled idea-grow code — Simulated modes, mid-run Lock refusal, blocked destination selection, frozen-install verification

- **R352 (§17)** Lock-aware read projection support used by GUI/Session may be built as internal work in slice 1; public MCP tool coverage and agent instructions remain slice 5. This keeps slices 2–5 independently buildable after the shared storage foundation.

- **R353 (§17)** Do not expose GUI lock control until the storage gate and trigger suite pass. No real user tree is locked automatically as part of release qualification.

- **R354 (§18.1)** Run core cases for both Topic and Literature trees, and for Owner and Agent where existing Workspace policy permits the requested write. Include Locks in both Workspaces.

- **R379 (§18.1)** Use a fixture helper that calls the same `lock_tx` under an edit lock and transaction. Keep it inside tests; do not ship a hidden CLI lock command. Tampering tests may deliberately remove a trigger on their own temporary database to exercise detection. They must never touch the user's configured archive.

- **R392 (§18.3)** Add focused unit tests in `the tree_lock module` for fingerprint equivalence, seal ordering, gate predicates, exact-set integrity, event deduplication and confirmation hashing. Use existing test infrastructure for public paths:

- **R393 (§18.3)** `the public CLI/service integration suite`: CLI, GUI API, migrations, delete-all registration exclusion and dump extraction/cleanup.

- **R394 (§18.3)** `the public Workspace/MCP integration suite` and the existing batch/preview suites: Access composition, typed refusals, aggregate previews, replay and complete rollback.

- **R395 (§18.3)** `the structural placement integration suite` / placement fixtures: insertion, mapping, pin and compaction effects, including temporary rename ordering.

- **R396 (§18.3)** Session unit/PTY tests: `[locked]`/`[pinned]`, refusal before edit, `del keep-root` and Pointer recovery.

- **R397 (§18.3)** Existing GUI JavaScript tests: action disabling, stale plan/draft retention and refresh.

- **R398 (§18.3)** Existing digest/grow harnesses: mode-specific destinations, no job dispatch for a known locked write target, locked source reading and mid-run refusal.

- **R399 (§18.3)** Run focused tests during implementation, then the repository's release checks once the slices are integrated:

- **R400 (§18.3)** Release commands: run cargo test --all-targets, standalone checker and digest-tool Cargo tests via their existing manifests, and the digest PowerShell run-all harness; also run affected GUI JavaScript and frozen Workflow assemble/verify checks.

- **R401 (§18.3)** Use the existing JavaScript and Workflow assemble/verify commands named in each affected wiki block. Keep service fixtures isolated with their scratch `ZT_CONFIG_DIR`, archive and GUI port. For refusals and rollback, compare actual Card/Link/metadata/receipt/lock-table state before and after, not only the returned message. For allowed incoming Links, compare every locked member's authored fingerprint and seal while permitting the expected generated Reverse section change.

- **R402 (§18.3)** The commands above are implementation acceptance requirements. They have not been run to claim delivery of this design-only feature.

- **R403 (§19)** Update documentation with the corresponding code slice, in the same commit as required by `the repository agent instructions`:

- **R404 (§19)** `the canonical domain glossary` — Add Lock, Locked tree, Pinned target, Emptied tree using the proposal's definitions; explain baseline Broken-link exception where appropriate

- **R405 (§19)** `the Service, CLI and storage wiki block` — Schema/version/trigger setup, connection functions, status, dump, delete commands, migration rules

- **R406 (§19)** `the Card operations wiki block` — Guard, typed refusal, delete scopes, complete mapping checks, batch previews/replay

- **R407 (§19)** `the Cards, Workspaces and Links wiki block` — Authored identity, Access composition, pins and generated Reverse-link allowance

- **R408 (§19)** `the GUI wiki block` — Lock/note/unlock routes, plan hash, controls, history, integrity and draft preservation

- **R409 (§19)** `the Session wiki block` — Marks, keep-root, prechecks, registration exclusion and refusal hints

- **R410 (§19)** `the MCP server wiki block` — Read fields, writable meaning, error serialization and preview limitations

- **R411 (§19)** `the agent-integration wiki block` — Owner-only policy and updated integrations

- **R412 (§19)** `the Digest workflows wiki block`, the Digest scripts block, the zt-digest block, the Idea-grow block — Preparation/destination rules and the specific newly published versions

- **R413 (§19)** `the dump/delete design`, README/help surfaces — `zt del --all`, keep-root, clear compatibility window and `locks.json`

- **R414 (§19)** Assign the new `the tree_lock module` to the existing Service, CLI and storage wiki block, which owns the database and migrations. Describe operation integration in Card operations and cross-link the blocks. Do not create an extra wiki block solely because there is a new file. Run the project-wiki check after code/doc integration; do not stamp unrelated stale blocks merely to make the check green.

- **R415 (§19)** For deployment, follow the existing zt restart/install procedure and retain the consistent pre-upgrade backup. Verify version 0.22.0, service health, Link integrity and Lock integrity with isolated qualification first. Restart agent clients and install the updated skill integrations. The owner then chooses any real tree to lock through the GUI. This design document itself changes no archive, Workflow install, production binary, existing proposal or current wiki behavior.

## Acceptance criteria

- [ ] **S18-AC01** All feature requirements and named T01–T24/D01–D12 acceptance scenarios have passing evidence at their appropriate public seam; compare whole relevant Card/Link/metadata/receipt/Lock state for refusals and rollback and authored seals for allowed generated updates.
- [ ] **S18-AC02** Run the complete root all-target tests, standalone checker tests, standalone digest-tool tests, digest PowerShell harness and affected GUI/Workflow JavaScript and assemble/verify suites after focused changes pass. Report actual results and any environment limitation; a green unrelated suite is not proof.
- [ ] **S18-AC03** Use isolated config/archive roots and GUI ports; no test or qualification locks, edits, empties or migrates the configured owner archive. Raw tampering remains confined to temporary fixtures.
- [ ] **S18-AC04** Add the four domain definitions; update storage/CLI, Card operations, Cards/Links, GUI, Session, MCP, agent integration, digest-script/tool/Workflow and idea-grow docs, plus README/help and dump/delete design, in the code commits that affect them.
- [ ] **S18-AC05** Assign the new tree_lock module to the existing storage wiki block and cross-link behavior; do not create a redundant block or stamp unrelated stale blocks. Run the wiki check and distinguish preexisting unrelated findings.
- [ ] **S18-AC06** Keep the KISS architecture and stated complexity: snapshot storage O(members + pin pairs + events), indexed ordinary checks, bulk selected annotations, selected-tree lock capture and full integrity only at named check points. Add no body copies, global per-read scan, new poller or generic framework.
- [ ] **S18-AC07** Qualify installed version 0.22.0, service health, Link/Lock integrity and consistent pre-upgrade backup/rollback with isolated fixtures and new client/integration versions. Older writers are unsupported against the upgraded archive.
- [ ] **S18-AC08** Retain the next-release removal of the clear notice as a documented follow-up. No restore command, partial/time-expiring Lock, force bypass, automatic digest Lock, historical bulk edit or production owner Lock operation is part of completion.
- [ ] **S18-AC09** Release evidence states the policy boundary honestly: deliberate token/SQL bypass and external source-document mutation are outside the promise; successful design/issue publication alone never establishes implementation or installed qualification.

**Verification seam:** Integrated public CLI/MCP/GUI/Session/Workflow fixtures, full release commands, wiki check and isolated installed-binary health/rollback verification.

**Delivery rule:** Keep the agreed scope and KISS architecture, preserve unrelated work and existing Access/domain rules, and update each affected wiki block in the same code commit. This ticket does not authorize a real-owner archive migration, digest rerun or Lock operation.

## Blocked by

- [S02 / #125](https://github.com/liudesheng9/zet/issues/125)
- [S05 / #128](https://github.com/liudesheng9/zet/issues/128)
- [S07 / #130](https://github.com/liudesheng9/zet/issues/130)
- [S08 / #131](https://github.com/liudesheng9/zet/issues/131)
- [S10 / #133](https://github.com/liudesheng9/zet/issues/133)
- [S12 / #135](https://github.com/liudesheng9/zet/issues/135)
- [S13 / #136](https://github.com/liudesheng9/zet/issues/136)
- [S17 / #140](https://github.com/liudesheng9/zet/issues/140)
