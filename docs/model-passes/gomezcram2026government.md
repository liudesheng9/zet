# gomezcram2026government: post-digest model passes

Gómez-Cram et al. (2026), *Government Funding Costs Under Financial Repression* (NBER conference paper f249580), IMEN1 queue paper 4. Digest run wf_1ae20ccf-a6e (2026-11-13, OpenCode Go), run report `complete: true`, 0 gaps (cleanup finished by hand). Post-digest sequence (`zt-post-digest`) on the live box at the owner's request, run by a side session of the IMEN1 queue (2026-11-14) while the queue digested other books on the same box. Stage subagents run on Opus at effort medium (owner).

## Stage 1: fold

Date 2026-11-14, live box. Backup: `D:\zt\backups\zt-before-gomezcram2026government-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 17 Model Cards (15 formal, 2 concept), 10 candidate groups (G2: 2\|a, 6\|a, 8\|a by rests-on; G7: 9\|a, 13\|a, 15\|a, 16\|a, 17\|a; G8: 11\|a, 17\|b; the rest single, many joined by **引入** imports of 2\|a, 4\|b and 5\|a parts).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 2\|a | §2–2.1.1 | baseline repression model | 2\|a | formal; asset holders and hand-to-mouth agents, Epstein-Zin preferences, the repression constraint and its wedge Ω in the bond pricing equation; first formal card, kept as head |
| 1\|c | §1 | baseline repression model | 2\|a | concept; the introduction's verbal version (two agent types, the third policy instrument, two channels, two monetary-fiscal regimes); becomes the intuition chapter |
| 4\|b | §2.3–2.4 | baseline repression model | 2\|a | formal; the policy block (Taylor rule, tax rule, perpetuity budget constraint, bond price with the wedge) of the same model; imported 2\|a's pricing equation |
| 5\|a | §2.5–2.7 | baseline repression model | 2\|a | formal; closes the same model (repression rule, mandatory holdings, market clearing, equilibrium definition) |
| 6\|a | §3.1 | baseline repression model | 2\|a | formal; the same model log-linearized, its determinacy regions; rests on 2\|a |
| 8\|a | §3.2.3 | baseline repression model | 2\|a | formal; the same model's risk-premium formula (36) under monetary accommodation; rests on 2\|a |
| 9\|a | §3.3.1 | baseline repression model | 2\|a | formal; the same model in the active/active region, where repression restores equilibrium |
| 15\|a | §4.1, B.1 | baseline repression model | 2\|a | formal; the same model in the fiscal-accommodation regime (a parameter region, not new primitives) |
| 11\|a | §4.2, B.3 | baseline repression model | 2\|a | formal; a variant that changes one assumption (repression ends with a probability) and keeps the rest |
| 12\|a | A.1 | baseline repression model | 2\|a | formal; the same model's full nonlinear equation system |
| 13\|a | A.2 | baseline repression model | 2\|a | formal; the same model's linearized solution and loadings in the four regimes |
| 16\|a | B.1.1 | baseline repression model | 2\|a | formal; the risk premium of the same model in the fiscal-accommodation regime |
| 17\|a | B.2 | baseline repression model | 2\|a | formal; the same model with both policies accommodating (equilibrium continuum) |
| 17\|b | B.3 | baseline repression model | 2\|a | formal; the policy functions of the stochastic-duration variant (rests on 11\|a) |
| 3\|a | §2.2.1–2.2.2 | micro-foundation of the wedge (mandatory holdings, capital regulation) | 3\|a | formal; its own question (where Ω comes from) and its own primitives (competitive intermediaries, regulation parameters); the baseline only borrows the wedge it derives: a micro-foundation stays a model of its own |
| 20\|a | C.1 | QE balance-sheet channel | 20\|a | formal; intermediaries with a leverage constraint (A25) derive the wedge from QE; own primitives and question |
| 4\|a | §2.2.3 | QE balance-sheet channel | 20\|a | concept; the main text's verbal version of 20\|a (QE lowers yields through the intermediaries' collateral constraint, 3 parts); becomes its intuition chapter |

Three models. The evidence settled every row; the two micro-foundation models are kept apart from the baseline by the skill's rule (a micro-foundation borrowed as an input does not make the same model) and from each other by their primitives (regulation versus an intermediary leverage constraint).

### Fold (executed 2026-11-14, live box)

Method: the per-card fold script of the assenza2026louder pass (`fold.mjs`, with a batch retry), driven in book order by `drive.mjs` (this session's scratchpad), one Card per run: strip **结构**/**引入**, mask every Link to a Model Card or part, park the parts beside the section anchor, retype (facet `第 N 章：…`, emphasis core, level cleared), move the head `alone` to the end of the head's main line, return the parts, restore the body with the rename map. Each former head's Idea-note Direct chain went to its section anchor (4\|b\|1–5, 5\|a\|1–7, 6\|a\|1–15, 8\|a\|1–3, 12\|a\|1–5, 13\|a\|1–7, 15\|a\|1–5, 16\|a\|1–4, 17\|a\|1–2, 17\|b\|1–3, 4\|a\|1). Every **引入** line of the folded heads (4\|b, 5\|a, 6\|a, 8\|a, 11\|a, 13\|a, 15\|a, 16\|a, 17\|a, 17\|b) named a part that is now 2\|a's own, so none was appended; 2\|a and 20\|a have no **引入** section. Every run returned ok.

| Former head | Now | Facet |
|---|---|---|
| 1\|c | 2\|a\|a\|1 | 第 1 章：文字版本与两条渠道（§1）; directly after the setting card |
| 4\|b | 2\|a\|a\|10 | 第 2 章：货币与财政政策块（§2.3–2.4） |
| 5\|a | 2\|a\|a\|11 | 第 3 章：抑制政策、市场出清与均衡（§2.5–2.7） |
| 6\|a | 2\|a\|a\|12 | 第 4 章：线性化系统与确定性区域（§3.1） |
| 8\|a | 2\|a\|a\|13 | 第 5 章：货币配合下的风险溢价（§3.2.3） |
| 9\|a | 2\|a\|a\|14 | 第 6 章：货币不配合时的抑制均衡（§3.3.1） |
| 15\|a | 2\|a\|a\|15 | 第 7 章：财政配合体制（§4.1、B.1） |
| 11\|a | 2\|a\|a\|16 | 第 8 章：抑制的随机持续时间（§4.2、B.3） |
| 12\|a | 2\|a\|a\|17 | 第 9 章：非线性方程组（A.1） |
| 13\|a | 2\|a\|a\|18 | 第 10 章：线性化均衡与四体制载荷（A.2） |
| 16\|a | 2\|a\|a\|19 | 第 11 章：财政配合体制的风险溢价（B.1.1） |
| 17\|a | 2\|a\|a\|20 | 第 12 章：双配合体制的连续统均衡（B.2） |
| 17\|b | 2\|a\|a\|21 | 第 13 章：随机持续时间的政策函数（B.3） |
| 4\|a | 20\|a\|a\|1 | 第 1 章：文字版本（§2.2.3）; directly after 20\|a's setting card |

2\|a's own derivation chain (budget constraint, repression constraint, wedge, pricing, CAPM limit) stays the setting's Direct chain (2\|a\|a\|2–9); the restructure makes it a chapter.

Gate:

- `model-survey.mjs gomezcram2026government`: 3 Model Cards, exactly the decided heads: 2\|a (formal, 146 parts), 3\|a (formal, 14), 20\|a (formal, 14).
- `check-model.mjs 2|a`: 146 members (22 main, 124 side); every former head a part; no closure or forward fault; `unanchored` only on the 13 new chapter cards (2\|a\|a\|1, 10–21); left to the restructure.
- `check-model.mjs 3|a`: 14 members (6 main, 8 shared: legacy （共用） shares of 2\|a's parts), loose 1, `self_sufficient` true; the import pass turns the shares into **引入** lines.
- `check-model.mjs 20|a`: 14 members (8 main, 3 side, 2 shared, 1 off), `unanchored` on the new chapter 20\|a\|a\|1, loose 2; left to the restructure.
- `list_broken_links` (agent): empty.
- Term relink: `{"rounds":1,"ok":true,"cards_changed":1,"links_added":2,"cards_refused":0,"links_repaired":0}`.
