# Slip-box implementation: PRD and issue coverage

Status: planning published, 2026-10-16. This index records specification coverage, not implementation completion.

Source: [implementation design](slip-box-luhmann-implementation-design.md), SHA-256 e5ed0b6249e7bf4a41dead07ed02ce68177ac162df77ce16f2837ed9c2b15f8f. Baseline: 51ba12d2460cb4d4bc1a8828f6f6798174ad7cf6. The source design was preserved unchanged while publishing this issue set.

PRD: [#50](https://github.com/liudesheng9/zet/issues/50).

## Delivery slices

| Slice | GitHub issue | Type | Dependencies | Acceptance criteria |
| --- | --- | --- | --- | --- |
| S01 | [Correct insertion and whole-Card Link guidance across ZT skills](https://github.com/liudesheng9/zet/issues/51) | AFK | None | 8 |
| S02 | [Make book attribution and Finding classification consistent in digest audits](https://github.com/liudesheng9/zet/issues/52) | AFK | None | 10 |
| S03 | [Generate bounded, frozen Bridge candidate jobs for an existing book](https://github.com/liudesheng9/zet/issues/53) | AFK | [S02 #52](https://github.com/liudesheng9/zet/issues/52) | 17 |
| S04 | [Publish checked, immutable Bridge reports from bounded judge jobs](https://github.com/liudesheng9/zet/issues/54) | AFK | [S03 #53](https://github.com/liudesheng9/zet/issues/53) | 18 |
| S05 | [Apply explicitly accepted new-book Bridges through guarded Card edits](https://github.com/liudesheng9/zet/issues/55) | AFK | [S04 #54](https://github.com/liudesheng9/zet/issues/54) | 15 |
| S06 | [Resume interrupted Bridge applies without duplicate or lost progress](https://github.com/liudesheng9/zet/issues/56) | AFK | [S05 #55](https://github.com/liudesheng9/zet/issues/55) | 10 |
| S07 | [Bridge mathematical units to earlier books with correct citation direction](https://github.com/liudesheng9/zet/issues/57) | AFK | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S06 #56](https://github.com/liudesheng9/zet/issues/56) | 10 |
| S08 | [Report actual semantic Bridge counts and unknown book isolation in audits](https://github.com/liudesheng9/zet/issues/58) | AFK | [S02 #52](https://github.com/liudesheng9/zet/issues/52) | 9 |
| S09 | [Run report-first Bridge discovery in book, economics, and math digests](https://github.com/liudesheng9/zet/issues/59) | AFK | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58) | 12 |
| S10 | [Apply identified Bridge reports through workflows without model probes](https://github.com/liudesheng9/zet/issues/60) | AFK | [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S09 #59](https://github.com/liudesheng9/zet/issues/59) | 8 |
| S11 | [Build and qualify frozen Bridge workflow versions and their documentation](https://github.com/liudesheng9/zet/issues/61) | AFK | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) | 13 |
| S12 | [Verify installed guidance and workflows with a real report-only Bridge pilot](https://github.com/liudesheng9/zet/issues/62) | AFK | [S11 #61](https://github.com/liudesheng9/zet/issues/61) | 10 |
| S13 | [Review and verify the explicitly accepted live Bridge apply pilot](https://github.com/liudesheng9/zet/issues/63) | HITL | [S12 #62](https://github.com/liudesheng9/zet/issues/62) | 11 |

## Design section coverage

Every section below has explicit contract and acceptance text in the linked issues. The issue bodies are self-contained and carry the source hash. Parent-section rows cover their introductory text; each subsection also has its own mapping. Original source schemas are expressed as field/ordering/validation requirements rather than pasted code examples.

| Design section | Source lines | Subject | Owning issues |
| --- | --- | --- | --- |
| 1 | 9–51 | 1. The implementation in one page | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 2 | 52–53 | 2. Requirements and boundaries | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 2.1 | 54–71 | 2.1 Required behavior | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 2.2 | 72–77 | 2.2 Features outside this change | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 2.3 | 78–81 | 2.3 Scope of this document | [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 3 | 82–107 | 3. Findings from the source review | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 4 | 108–111 | 4. C1: exact skill changes | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 4.1 | 112–125 | 4.1 The address explanation | [S01 #51](https://github.com/liudesheng9/zet/issues/51) |
| 4.2 | 126–139 | 4.2 Placement must agree with insertion | [S01 #51](https://github.com/liudesheng9/zet/issues/51) |
| 4.3 | 140–147 | 4.3 The whole-Card Link rule | [S01 #51](https://github.com/liudesheng9/zet/issues/51) |
| 4.4 | 148–153 | 4.4 C1 verification | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 5 | 154–180 | 5. C2: one compiler with four steps | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 5.1 | 181–200 | 5.1 Workflow arguments | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 5.2 | 201–214 | 5.2 Mode behavior | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 5.3 | 215–244 | 5.3 Script results | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 6 | 245–246 | 6. Shared Card classification and book attribution | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S08 #58](https://github.com/liudesheng9/zet/issues/58) |
| 6.1 | 247–261 | 6.1 A small pure helper | [S02 #52](https://github.com/liudesheng9/zet/issues/52) |
| 6.2 | 262–277 | 6.2 Attribution rules | [S02 #52](https://github.com/liudesheng9/zet/issues/52) |
| 6.3 | 278–287 | 6.3 Attribution is a reporting approximation | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S08 #58](https://github.com/liudesheng9/zet/issues/58) |
| 7 | 288–289 | 7. Planning and candidate selection | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 7.1 | 290–297 | 7.1 Read the archive once for selection | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 7.2 | 298–312 | 7.2 Source Cards: book profile | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 7.3 | 313–326 | 7.3 Source Cards: math profile | [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 7.4 | 327–340 | 7.4 Eligible candidate targets | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 7.5 | 341–358 | 7.5 Reuse relatedness and add literal term matches | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 7.6 | 359–368 | 7.6 Read full endpoints and make jobs | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 8 | 369–372 | 8. Files and schemas | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 8.1 | 373–394 | 8.1 Staging layout | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 8.2 | 395–423 | 8.2 Endpoint record | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S05 #55](https://github.com/liudesheng9/zet/issues/55) |
| 8.3 | 424–455 | 8.3 Plan record | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 8.4 | 456–493 | 8.4 Judge answer: one result per source | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 8.5 | 494–513 | 8.5 Published proposals | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55) |
| 8.6 | 514–521 | 8.6 Apply state | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| 9 | 522–523 | 9. Relation and direction rules | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 9.1 | 524–541 | 9.1 Allowed words | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 9.2 | 542–554 | 9.2 Carrier constraints | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 9.3 | 555–569 | 9.3 Earlier-carrier exception | [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 9.4 | 570–575 | 9.4 Model preservation | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 10 | 576–577 | 10. Judge job and precheck | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 10.1 | 578–595 | 10.1 Judge instructions | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 10.2 | 596–617 | 10.2 Exact answer validation | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 10.3 | 618–643 | 10.3 Offline language checks | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 11 | 644–671 | 11. The Bridge report | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 12 | 672–673 | 12. Applying accepted proposals | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 12.1 | 674–681 | 12.1 Acceptance validation before writes | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 12.2 | 682–697 | 12.2 Revalidate endpoints without relocation guesses | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| 12.3 | 698–711 | 12.3 Group edits by carrier | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| 12.4 | 712–734 | 12.4 Make the existing guarded edit | [S05 #55](https://github.com/liudesheng9/zet/issues/55) |
| 12.5 | 735–750 | 12.5 Retries and interrupted writes | [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 12.6 | 751–766 | 12.6 Verification after writes | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 12.7 | 767–776 | 12.7 Concurrency limit | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 13 | 777–803 | 13. Audit counts | [S08 #58](https://github.com/liudesheng9/zet/issues/58) |
| 14 | 804–805 | 14. Workflow integration | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 14.1 | 806–822 | 14.1 Shared framework stage | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 14.2 | 823–842 | 14.2 Module control flow | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 14.3 | 843–854 | 14.3 Roles and resource bounds | [S09 #59](https://github.com/liudesheng9/zet/issues/59) |
| 14.4 | 855–862 | 14.4 The handwritten book workflow | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 14.5 | 863–882 | 14.5 Results, versions, and frozen installation | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 15 | 883–901 | 15. Failure and recovery policy | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 16 | 902–907 | 16. Tests with concrete outcomes | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 16.1 | 908–920 | 16.1 Shared attribution tests | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 16.2 | 921–954 | 16.2 Bridge compiler tests | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 16.3 | 955–967 | 16.3 Audit tests | [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 16.4 | 968–983 | 16.4 Workflow harnesses | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 16.5 | 984–999 | 16.5 Verification commands | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 17 | 1000–1029 | 17. Implementation sequence and file ownership | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 18 | 1030–1031 | 18. Rollout and user examples | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 18.1 | 1032–1046 | 18.1 Install after tests | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 18.2 | 1047–1066 | 18.2 A report-only pilot | [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 18.3 | 1067–1089 | 18.3 Apply the reviewed report | [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 18.4 | 1090–1095 | 18.4 Hubs and later maintenance | [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 19 | 1096–1111 | 19. Completion criteria | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 20 | 1112–1121 | 20. Why this remains the simplest structure | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| Sources checked | 1122–1142 | Sources checked | [S11 #61](https://github.com/liudesheng9/zet/issues/61) |

## Named artifacts and gates

| Artifact or contract | Owning issues |
| --- | --- |
| book-of.ps1 | [S02 #52](https://github.com/liudesheng9/zet/issues/52) |
| bridge-compiler.ps1 | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| dsh-bridge-instructions.zh.md | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| zt-bridge.js | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| plan.json | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| proposals.json | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55) |
| state.json | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| report.md | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| apply.lock | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| read-only.allow.txt | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| book-of.tests.ps1 | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| bridge-compiler.tests.ps1 | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| digest-audit.tests.ps1 | [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| requires-version.tests.ps1 | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| workflow-harness.mjs | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| framework-harness.mjs | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| racing-zt-mcp | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| install-agent-integrations.ps1 | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| NoProfile | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| ZT_CONFIG_DIR | [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| ZT_GUI_PORT=0 | [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| L1,L2,R2,R3,R4,R5,R7,R9 | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| BR1 | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| BR9 | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 2.0.0→2.1.0 | [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 3.0.0→3.1.0 | [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 2.1.0→2.2.0 | [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| Source-level tests | [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| installed-file hashes | [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| report-only pilot | [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| accepted live apply pilot | [S13 #63](https://github.com/liudesheng9/zet/issues/63) |

| Named gate | Required evidence |
| --- | --- |
| book-of.tests.ps1 | PowerShell NoProfile/File; attribution/categories/exact identity |
| bridge-compiler.tests.ps1 | PowerShell NoProfile/File; plan/review/apply/recovery/math |
| digest-audit.tests.ps1 | PowerShell NoProfile/File; actual semantic pairs and incomplete reads |
| requires-version.tests.ps1 | PowerShell NoProfile/File; new 7.5 requirements |
| workflow-harness.mjs | Node; handwritten workflow behavior |
| framework-harness.mjs | Node; economics/math graph, conditions, roles, frozen builds |
| project-wiki check | Node wiki.mjs check; all affected functional blocks |
| assembler install/verify | Project scope; both variants; base/preflight/version/installed equals built |
| skill install/hash/new conversation | Actual installed source equality and loaded guidance |
| report-only pilot | Actual installed run with fresh output, real key, bounded sources, no body edits |
| accepted-apply pilot | Actual human report-bound selection, backup, quiet filing, guarded effects/retry/model/audit verification |

The seven source commands remain the named gates: run the four PowerShell suites with NoProfile and File, both Node harnesses, and Node project-wiki check. Assembler verify, installed hashes/new client guidance, and both real pilots are additional completion gates. This planning task does not claim to have run future feature tests or pilots.

## User story coverage

| PRD story | Implementation slices |
| --- | --- |
| 1 | [S01 #51](https://github.com/liudesheng9/zet/issues/51) |
| 2 | [S01 #51](https://github.com/liudesheng9/zet/issues/51) |
| 3 | [S01 #51](https://github.com/liudesheng9/zet/issues/51) |
| 4 | [S01 #51](https://github.com/liudesheng9/zet/issues/51), [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 5 | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S08 #58](https://github.com/liudesheng9/zet/issues/58) |
| 6 | [S02 #52](https://github.com/liudesheng9/zet/issues/52) |
| 7 | [S02 #52](https://github.com/liudesheng9/zet/issues/52) |
| 8 | [S02 #52](https://github.com/liudesheng9/zet/issues/52), [S08 #58](https://github.com/liudesheng9/zet/issues/58) |
| 9 | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 10 | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S09 #59](https://github.com/liudesheng9/zet/issues/59) |
| 11 | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 12 | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 13 | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 14 | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 15 | [S03 #53](https://github.com/liudesheng9/zet/issues/53) |
| 16 | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S09 #59](https://github.com/liudesheng9/zet/issues/59) |
| 17 | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 18 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 19 | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 20 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 21 | [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 22 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 23 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 24 | [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 25 | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 26 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S07 #57](https://github.com/liudesheng9/zet/issues/57) |
| 27 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S08 #58](https://github.com/liudesheng9/zet/issues/58), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 28 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 29 | [S04 #54](https://github.com/liudesheng9/zet/issues/54), [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 30 | [S04 #54](https://github.com/liudesheng9/zet/issues/54) |
| 31 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 32 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 33 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 34 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 35 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56) |
| 36 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 37 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 38 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 39 | [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 40 | [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 41 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 42 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 43 | [S05 #55](https://github.com/liudesheng9/zet/issues/55), [S06 #56](https://github.com/liudesheng9/zet/issues/56), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 44 | [S08 #58](https://github.com/liudesheng9/zet/issues/58) |
| 45 | [S08 #58](https://github.com/liudesheng9/zet/issues/58) |
| 46 | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 47 | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 48 | [S10 #60](https://github.com/liudesheng9/zet/issues/60) |
| 49 | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 50 | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S10 #60](https://github.com/liudesheng9/zet/issues/60), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 51 | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 52 | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 53 | [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 54 | [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |
| 55 | [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62) |
| 56 | [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 57 | [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 58 | [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 59 | [S11 #61](https://github.com/liudesheng9/zet/issues/61), [S12 #62](https://github.com/liudesheng9/zet/issues/62), [S13 #63](https://github.com/liudesheng9/zet/issues/63) |
| 60 | [S03 #53](https://github.com/liudesheng9/zet/issues/53), [S07 #57](https://github.com/liudesheng9/zet/issues/57), [S09 #59](https://github.com/liudesheng9/zet/issues/59), [S11 #61](https://github.com/liudesheng9/zet/issues/61) |

## Publication verification

The publication receipt re-fetches the live issue title, full body, state, labels, parent link, acceptance criteria, and real blocker links for the PRD and all thirteen slices. All issue bodies must match the authored payload after newline normalization. All criteria remain unchecked. Twelve slices use ready-for-agent and the live acceptance slice uses ready-for-human; the PRD uses ready-for-agent. No unrelated closed issue was changed.

Publishing this plan completes the PRD/issue-authoring request. Future implementation is complete only after the independent source, installed, and live acceptance evidence in S11–S13 exists.
