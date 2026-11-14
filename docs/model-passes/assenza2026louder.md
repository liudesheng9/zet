# assenza2026louder: post-digest model passes

Assenza et al. (2026), *Louder than Rates* (NBER conference paper f249548), IMEN1 queue paper 3. Digest run wf_c8b337b2-d89 (2026-11-13, OpenCode Go), run report `complete: true`, 0 gaps. Post-digest sequence (`zt-post-digest`) on the live box at the owner's request, run by a side session of the IMEN1 queue (2026-11-13/14) while the queue digested other books on the same box. Subagents run at effort medium (owner).

## Stage 1: fold

Date 2026-11-14, live box. Backup: `D:\zt\backups\zt-before-assenza2026louder-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 10 Model Cards (6 formal, 3 empirical, 1 concept), 10 candidate groups (no rests-on Link joined two Model Cards; 10\|a imported nine parts of 9\|a).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 7\|a | §3.1 | CRANK | 7\|a | formal; the household and information block of the paper's one theory model (incomplete-information NK with a communication rule); first formal card of §3, kept as head |
| 2\|b | 引言 | CRANK | 7\|a | concept; the introduction's verbal version of the same model (same agents, shocks, signal, two rules, the three results); becomes the intuition chapter |
| 8\|a | §3.1.2 | CRANK | 7\|a | formal; the firms and Rotemberg pricing block of the same model; same agents and timing |
| 8\|b | §3.1.2 | CRANK | 7\|a | formal; the monetary authority block (Taylor rule (23), communication rule (24)) of the same model |
| 9\|a | §3.2 | CRANK | 7\|a | formal; derives the same model's dynamic IS equation from the household's Euler equation under the signal of 7\|a |
| 10\|a | §3.3 | CRANK | 7\|a | formal; solves the same model's four-equation system (determinacy, loadings, IRFs, ZLB); imported 9\|a's IS equation |
| 5\|a | §2.2 | Fed communication reaction function | 5\|a | formal; the estimable partial-adjustment rule (3)–(4) for the change in statement entropy; its own question; estimated by 6\|a (a theory and its estimation are never folded; as velic2025inflation 3\|a / 8\|a) |
| 4\|a | §2.1 | entropy measure of communication effort | 4\|a | empirical; the measurement and its descriptive facts (Table 1, Figure 1) built for the whole paper: the series feeds both 6\|a and 6\|a\|28, so it cannot belong to one estimation (as velic2025inflation 7\|a) |
| 6\|a | §2.2 | US rule estimation (iterated GMM) | 6\|a | empirical; estimates 5\|a on US data 1994–2022 |
| 6\|a\|28 | §2.3 | cross-country rule estimation | 6\|a\|28 | empirical; the same question on a different data set (five other central banks, own extensions (9)): an empirical model of its own |

Five models. The evidence settled every row (no owner question).

### Fold (executed 2026-11-14, live box)

Method: the per-card fold script of the bergin2026exchange pass (`foldb.mjs`; copy `fold.mjs` in this session's scratchpad), one Card per run, previewed, then committed: strip **结构**/**引入**, mask every Link to a Model Card or part, park the parts beside the section anchor, retype (facet `第 N 章：…`, emphasis core, level cleared), move the head `alone` into 7\|a's main line, return the parts, restore the body with the rename map. 10\|a's Idea-note Direct chain (10\|a\|1–24) went to section 10 (now `10|b` …). 10\|a's **引入** line (`整块` 9\|a\|a\|8, the IS equation (30)) now names a part of the head and was not appended; 7\|a has no **引入** section.

Incident: the 9\|a run's final batch (move the head into the main line, return its parts) failed at commit with no reply (the queue session wrote the box at the same moment); the retype and the park had gone through and the restore wrote the unmasked body. The same batch was rerun at once and applied. The script now retries a batch up to four times.

| Former head | Now | Facet |
|---|---|---|
| 2\|b | 7\|a\|a\|1 | 第 1 章：文字版本与直觉（引言）; directly after the setting card |
| 8\|a | 7\|a\|a\|13 | 第 2 章：厂商与定价（§3.1.2） |
| 8\|b | 7\|a\|a\|14 | 第 3 章：货币当局的两条规则（§3.1.2） |
| 9\|a | 7\|a\|a\|15 | 第 4 章：不完全信息下的动态 IS 方程（§3.2） |
| 10\|a | 7\|a\|a\|16 | 第 5 章：均衡动态（§3.3） |

7\|a's own household and information block stays the setting's Direct chain (7\|a\|a\|2–12); the restructure makes it a chapter.

Gate:

- `model-survey.mjs assenza2026louder`: 5 Model Cards, exactly the decided heads: 7\|a (formal, 83 parts), 5\|a (formal, 6), 4\|a (empirical, 31), 6\|a (empirical, 52), 6\|a\|28 (empirical, 32).
- `check-model.mjs 7|a`: 83 members (17 main, 66 side); every former head a part; six `forward` faults (chapter 7\|a\|a\|14 names its own parts with `依赖` lines) and `unanchored` on the chapter cards 7\|a\|a\|1, 13, 14, 15: left to the restructure. 5\|a, 4\|a, 6\|a, 6\|a\|28: no fault, `self_sufficient` true (loose 0, 2, 2, 0).
- `list_broken_links` (agent): empty.
- Term relink: `{"rounds":1,"ok":true,"cards_changed":3,"links_added":3,"cards_refused":0,"links_repaired":0}`.
