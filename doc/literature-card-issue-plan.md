# Literature Card Issue Plan

Parent PRD: [#27](https://github.com/liudesheng9/zet/issues/27)

Source design: [literature-card-design.md](literature-card-design.md)

Status: breakdown approved by the user, implemented, verified, and closed through
GitHub issues #27-#34. Final implementation and live-migration proof is recorded
in [literature-card-implementation-evidence.md](literature-card-implementation-evidence.md).

## Published Issues

- [#27 [PRD] Literature Cards with BibTeX metadata and Citation-key identity](https://github.com/liudesheng9/zet/issues/27)
- [#28 Create and persist Literature Cards through the shell](https://github.com/liudesheng9/zet/issues/28)
- [#29 Create, navigate, and list Literature Cards in a Session](https://github.com/liudesheng9/zet/issues/29)
- [#30 Cite Literature Cards across the Card graph](https://github.com/liudesheng9/zet/issues/30)
- [#31 Edit Literature metadata and rename Citation keys safely](https://github.com/liudesheng9/zet/issues/31)
- [#32 Delete and clear Literature Cards without Location topology](https://github.com/liudesheng9/zet/issues/32)
- [#33 Dump Literature Markdown and exact Citation-key BibTeX files](https://github.com/liudesheng9/zet/issues/33)
- [#34 Migrate and verify the live ZT database for Literature Cards](https://github.com/liudesheng9/zet/issues/34)

## Approved Vertical Slices

1. [#28 Create and persist Literature Cards through the shell](https://github.com/liudesheng9/zet/issues/28)
   - Type: AFK
   - Blocked by: None
   - User stories covered: 1-24, 45-47, 77-79, and the shell/storage portions of 83-84
   - Delivers the constrained unified schema, Citation-key rules, BibTeX parser/title generation, atomic two-stage shell creation, counts, and legacy-schema refusal as one verifiable path.

2. [#29 Create, navigate, and list Literature Cards in a Session](https://github.com/liudesheng9/zet/issues/29)
   - Type: AFK
   - Blocked by: #28
   - User stories covered: 25-30, 37-47, and the Session portions of 83-85
   - Delivers real Session creation, validation retry, reading view, Pointer navigation, ROOT/Topic/Literature listing contexts, help, and counts with PTY coverage.

3. [#30 Cite Literature Cards across the Card graph](https://github.com/liudesheng9/zet/issues/30)
   - Type: AFK
   - Blocked by: #28 and #29
   - User stories covered: 38-56 and the graph portions of 83-84
   - Delivers the dual-address target resolver, all source/target Link combinations, Topic outbound Links, Reverse links, Broken links, `lsbk`, highlighting, and mouse navigation.

4. [#31 Edit Literature metadata and rename Citation keys safely](https://github.com/liudesheng9/zet/issues/31)
   - Type: AFK
   - Blocked by: #29 and #30
   - User stories covered: 28-36, 57-62, and the edit/rename portions of 83-85
   - Delivers the numbered Session edit choice, shell `--part`, metadata/text saves, generated-title repair, collision-safe Citation-key rename, confirmation, cross-kind Link rewriting, and atomic regeneration.

5. [#32 Delete and clear Literature Cards without Location topology](https://github.com/liudesheng9/zet/issues/32)
   - Type: AFK
   - Blocked by: #29, #30, and #31
   - User stories covered: 54-56 and 63-69 plus destructive-workflow regression portions of 84
   - Delivers single-card Literature deletion, Broken inbound Links, Pointer recovery, topology-command refusal, and full `zt clear` integration without a `clean` alias.

6. [#33 Dump Literature Markdown and exact Citation-key BibTeX files](https://github.com/liudesheng9/zet/issues/33)
   - Type: AFK
   - Blocked by: #28
   - User stories covered: 70-76 and archive portions of 83-84
   - Delivers YAML-front-matter Literature Markdown, exact `.bib` entries, locked hash input, mixed mapping/entry ordering, count semantics, and compression/lifecycle regression coverage.

7. [#34 Migrate and verify the live ZT database for Literature Cards](https://github.com/liudesheng9/zet/issues/34)
   - Type: AFK
   - Blocked by: #28, #29, #30, #31, #32, and #33
   - User stories covered: 79-85 and final evidence for the complete PRD
   - Delivers the coding-agent-owned live backup/migration, exact preservation proof, SQLite validation, installed-binary verification, full automated gate, real dump, and manual TUI smoke.

## Locked-Design Coverage Audit

| Locked design section | PRD representation | Child issue evidence |
| --- | --- | --- |
| Source Authority | Further Notes names the locked design and glossary; Implementation Decisions preserve inherited contracts | Every child links #27 and the source design; #28-#34 scope changes against existing behavior |
| Conflict Precedence | Solution, final Implementation Decision, and Further Notes limit supersession to explicit Literature behavior | #28-#34 carry focused non-goals and Topic/Regular regression criteria |
| Current Code Facts | Problem Statement and Solution describe the Location-only baseline and required extension | #28 owns storage/startup, #29 Session, #30 Links, #32 clear, and #33 dump baseline changes |
| Locked Requirements | Stories 1-85 plus all Implementation and Testing Decisions | The complete contract is partitioned across #28-#34 |
| Locked SQLite Shape | Stories 1-8 and 77-79; first seven Implementation Decisions | #28 implements and tests the constrained unified table; #34 deploys it to the live database |
| Locked Navigation, Listing, and Topology | Stories 37-47 and 63-66; navigation/count/target/deletion decisions | #29 owns navigation and listing; #32 owns topology refusal and deletion |
| Locked Reading and Broken-Link Behavior | Stories 37-39 and 48-56; reading-view and shared-Link decisions | #29 owns the view; #30 owns target rendering, activation, Broken links, Reverse links, and `lsbk`; #32 verifies deletion consequences |
| Locked Clear Behavior | Stories 67-69; clear decision | #32 owns the only-`clear`, all-kind reset, count, lock, Session, config, dump, and transaction contract |
| Locked BibTeX Interpretation | Stories 9-18; parser/title decisions | #28 owns parser, validation, raw preservation, and unit coverage; #31 reuses them for edits |
| Locked Literature Markdown Envelope | Stories 70-76; YAML, exact-text, hash, and `.bib` decisions | #33 owns byte-exact serialization, round-trip tests, hashing, and archive output |
| Locked Schema Compatibility Boundary | Stories 79-82; compatibility and live-migration decisions | #28 owns non-mutating legacy refusal; #34 owns backup, transactional live migration, proof, and recovery |
| Locked Literature Body and Generated Title | Stories 14, 17, 30, 35-36, and 49-52; body scanning and generated-zone decisions | #30 owns cross-kind body Links; #31 owns title/Reverse-link repair on both edit paths |
| Locked Literature Listing and Creation Completion | Stories 24, 27, and 40-47; creation-output and list/count decisions | #28 owns shell completion output; #29 owns Pointer outcome and all three listing contexts |
| Locked Edit Selection and Temporary Files | Stories 19-36; editor, retry, selection, cancellation, and lock decisions | #28 owns shell creation files/retry; #29 owns Session creation; #31 owns both edit-selection paths |
| Locked Dump Ordering | Stories 70-76; mixed mapping/archive ordering decisions | #33 owns mapping-key order, Markdown order, `.bib` order, determinism, and counts |
| Locked Shell Target Boundary | Stories 31-34, 38, and 63-66; target/help/topology decisions | #29 updates Session help and `go`; #31 owns `e`; #32 owns `del` and Location-only command refusal |
| Locked Shell Literature Edit Option | Stories 31-34; exact conditional `--part` decision | #31 owns `metadata|text`, all invalid forms, Location rejection, help, and editor-launch boundary |
| Locked Citation-Key Case Behavior | Stories 7-8 and 58; uniqueness/lookup decisions | #28 owns exact-case lookup and case-folded uniqueness; #31 owns rename collision checks; #33 preserves exact filenames/mappings |
| Acceptance and Verification | Stories 83-85 and Testing Decisions | #28-#33 each own seam-appropriate tests; #34 owns full suite, live evidence, real dump, and manual TUI smoke |
| KISS Implementation Boundary | Story 77, Out of Scope, and final Implementation Decision | #28-#34 use the shared table/graph/editor/dump seams and explicitly exclude parallel systems and unrelated refactors |
| Open Decisions | Further Notes marks every child AFK-ready; this plan records user approval | #28-#34 carry `ready-for-agent`; #34 proceeds without a user-run migration step |

Every locked design section maps to at least one numbered PRD story, an implementation/testing decision, and one published child issue. No design branch is left only in the source design document.

## Dependency Graph

```text
#28 shell/storage tracer
  +--> #29 Session creation/navigation
  |      +--> #30 Link graph
  |             +--> #31 edit/rename
  |                    +--> #32 delete/clear
  |
  +--> #33 dump

#28 + #29 + #30 + #31 + #32 + #33 --> #34 live migration and final verification
```

## Approval and Acceptance Gate

- Parent and all child issues carry `ready-for-agent`; this is the repository's approved AFK-ready state.
- All slices are AFK because the design contains no open decisions and the user explicitly assigned the live migration to the coding agent.
- Each implementation slice owns behavior-level tests at the highest existing seam; there is no horizontal test-only issue.
- Final acceptance is owned by #34 and requires all blockers complete, `cargo check`, full `cargo test`, recorded live migration evidence, one real dump, and one manual local TUI smoke.
