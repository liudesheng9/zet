# riboni2026presidential: post-digest model passes

Riboni and Ruge-Murcia, *Presidential Appointments to the Federal Reserve Board*, Work 29956. The digest is settled with one gap the owner accepted (audit/book@3); see `D:\zt-digest\_queue\imen1-queue.md`. All passes ran on the live box (`D:\zt`), 2026-11-14.

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-riboni2026presidential-fold.sqlite3` (integrity_check ok).

The survey found 11 Model Cards (1 concept, 10 empirical) in 11 candidate groups: no rests-on Link joined any two. Two rows needed judgement (8\|b and 9\|a). The proposal was shown to the owner, who approved it on 2026-11-14 ("Yes, fold as proposed").

| Model Card | Level | Model | Surviving head | Reason |
|---|---|---|---|---|
| 4\|a | empirical | appointment effect (LP-IV) | 4\|a | §3.1–3.2: the estimand (effect of the governors' share G on outcomes h meetings later), local projections with 2SLS and four external instruments |
| 3\|a | concept | appointment effect | 4\|a | the three-valued coding of the policy variable: the measurement built for this model's outcome |
| 3\|b | empirical | appointment effect | 4\|a | the meeting-level data (391 regular FOMC meetings) the model uses |
| 5\|a | empirical | appointment effect | 4\|a | the baseline result of the same estimand, data and instruments |
| 5\|a\|4 | empirical | appointment effect | 4\|a | heterogeneity: the share split by the appointing party |
| 6\|a | empirical | appointment effect | 4\|a | heterogeneity: appointments joining in election years |
| 7\|a | empirical | appointment effect | 4\|a | heterogeneity: interaction with the president's party |
| 8\|a | empirical | appointment effect | 4\|a | mechanism: the same instruments, the outcome is the members' policy stance |
| 8\|c | empirical | appointment effect | 4\|a | mechanism: the same instruments, the outcome is dissents |
| 9\|a | empirical | appointment effect | 4\|a | §5: the same appointment shock on the same meetings, estimated with another estimator family (SVAR-IV, departures as external instrument) on inflation and output |
| 8\|b | empirical | reaction function | 8\|b | §4.2: a different estimand (the association of stated stance with the same meeting's decision, no instrument); it takes the stance measure from 8\|a |

The book holds **2 models**: `4|a` and `8|b`, both empirical.

### Fold (executed 2026-11-14, live box)

Three runs of the fold script, re-reading the tree between runs (a fold removes a Side slot and renames later siblings, so later siblings were folded first):

- Main line of 4\|a after the fold: `4|a|a` data; `4|a|a|1` 数据章：三值政策变量的编码 (was 3\|a); `|2` 数据章：会议层面数据与政策变量 (was 3\|b); `|3` design; `|4` baseline specification; `|5`–`|6` the existing chapters 1–2; `|7` 第 3 章：基准局部 IV 投影 (was 5\|a); `|8` 第 4 章：党派分组的局部投影 (was 5\|a\|4); `|9` 第 5 章：选举年的局部 IV 投影 (was 6\|a); `|10` 第 6 章：与总统党派的交互 (was 7\|a); `|11` 第 7 章：政策立场的局部 IV 投影 (was 8\|a); `|12` 第 8 章：异议的局部投影 (was 8\|c); `|13` 第 9 章：SVAR-IV 与宏观结果 (was 9\|a).
- Each folded card's data and limitation parts are filed under it. Idea notes that were Direct successors of a folded card (5\|a\|1–5, 6\|a\|1–3, 7\|a\|1–3) moved to their section as Side branches; the idea Side cards 3\|a\|a and 6\|a\|a stay in their sections.

### Gate

- `model-survey.mjs`: exactly 2 Model Cards, 4\|a (185 parts) and 8\|b (25 parts). One `false share` from 8\|b to a part of 4\|a (its Link to the stance model, former 8\|a): left to the import stage.
- `check-model.mjs 8|b`: 25 members (1 shared), `self_sufficient` true. `check-model.mjs 4|a`: every former head is a part; the remaining faults are the unanchored folded chapters (`4|a|a|1`, `|2`, `|7`–`|13`) and 9 loose parts, which the settings and restructure passes fix.
- `list_broken_links`: empty. `term-relink.ps1`: `{"links_added":9,"cards_changed":6,"ok":true}`.

### Open

- Every folded chapter carries its own data card, restating the 391 meetings; the settings pass merges them into one data tree.
- 8\|b's Link to the stance measure becomes an **引入** line in stage 4.
