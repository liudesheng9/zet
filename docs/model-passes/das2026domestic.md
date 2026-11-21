# das2026domestic: post-digest model passes

Das et al. (2026), *Domestic Debt Restructuring, Central Bank, and Monetary Policy in EMDEs: Lessons from Recent Country Cases*, IMEN1 queue paper 10. The digest run report is `complete: true` (Cited list rebuilt by mode cited-cards). Post-digest sequence (`zt-post-digest`) on the live box at the owner's request (2026-11-21, "Yes, run Das", after the Furceri sequence).

Open owner decision carried from the digest queue: merge Das 8\|a\|12 into 8\|a\|11? This sequence does not decide it.

## Stage 1: fold

Date 2026-11-21, live box. Backup: `D:\zt\backups\zt-before-das2026domestic-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 5 Model Cards (4 concept, 1 empirical), 5 candidate groups; no rests-on Link joined any two (`read` pairs: 5\|b→4\|a kin, 11\|a→3\|a and 4\|a kin, 13\|a→5\|b compare).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 3\|a | §II.B | why a DDR weakens monetary policy (three balance sheets, four channels) | 3\|a | concept; the three-balance-sheet framework (one security G on the sovereign, central-bank and bank balance sheets; the lock makes net saving smaller than the gross haircut; the lock acts through four channels); first card of the model, kept as head |
| 4\|a | §III | same | 3\|a | concept; the same question and primitives continued: the four channels that 3\|a names and says it leaves for later (`四渠道的逐条展开不在本模型内`) |
| 5\|b | §IV | DDR event study of four monetary indicators | 5\|b | empirical; median paths of base money, broad money, the money multiplier and the reserve ratio around 17 DDR cases (t−3 to t+3) |
| 13\|a | 附录 | same | 5\|b | concept; the coding table of the same 17 DDR events (years, crisis type R/E/B, monetary regime): the sample and grouping built for 5\|b, as riboni 3\|a (coding of the policy variable) |
| 11\|a | §V | DDR design: relief against the central bank's viability | 11\|a | concept; own question (how to design a restructuring), borrows only the accounting of 3\|a and two of its channels (`对应` Links) |

The book holds **3 models**: `3|a` (concept), `5|b` (empirical) and `11|a` (concept). The evidence settled every row by the skill's rules.

### Fold (executed 2026-11-21, live box)

Same fold script as furceri2026divide (`fold.mjs`/`drive.mjs`, this session's scratchpad), one run per card: 4\|a went `after 3|a|a|2` (the end of 3\|a's main line), 13\|a went `after 5|b|b` (the top of 5\|b's main line, after the data card). The Direct idea chains 4\|a\|1–12 and 13\|a\|1–5 moved to their section anchors 4 and 13 as Side branches. No folded head had an **引入** line. Both runs returned ok.

| Former head | Now | Facet |
|---|---|---|
| 4\|a | 3\|a\|a\|3 | 第 1 章：四个渠道（§III） |
| 13\|a | 5\|b\|b\|1 | 数据章：17 次重组事件的编码表（附录） |

### Gate

- `model-survey.mjs`: 3 Model Cards, `3|a` (8 parts), `5|b` (38 parts), `11|a` (5 parts), exactly the decided heads.
- `check-model.mjs`: every former head is a part. `3|a` loose 1, `self_sufficient` false (the new chapter has no rests-on line to the setting); `5|b` loose 2, `self_sufficient` false (left to the settings and restructure passes); `11|a` self-sufficient, loose 1.
- Tree 398 Cards before and after; `list_broken_links` empty. Term relink: 0 changes.

### Which models take the later stages

By the sequence skill, a concept model takes the fold only; stages 2–5 run on a larger concept model only on the owner's word. So `3|a` (8 parts) and `11|a` (5 parts) stop here, and their loose parts stay as the fold left them. `5|b` takes stages 2, 3, 4 and 6; stage 5 (illustrate) does not apply, since the book has no formal model. Stage 7 runs on the whole tree.
