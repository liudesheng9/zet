# arrocannarsa2026flight: post-digest model passes

Arro-Cannarsa (2026), *Flight to Safety: Monetary Policy in an Open Economy*, IMEN1 queue paper 5. Digest run wf_f3ec6ae3-25c plus mode term-link wf_0c70dc55-951 (2026-11-13, OpenCode Go), run report `complete: true`, 0 gaps. Post-digest sequence (`zt-post-digest`) on the live box at the owner's request, run by a side session of the IMEN1 queue (2026-11-14) while the queue digested other books on the same box. Stage subagents run on Opus at effort medium (owner).

Open owner decision carried from the digest: Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a record the same 2009-03-12 Swiss National Bank FX intervention. Merge them? This sequence does not decide it; the Result section gives their final addresses.

## Stage 1: fold

Date 2026-11-14, live box. Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 22 Model Cards (20 formal, 2 empirical), 16 candidate groups (G1: 2\|a, 3\|a, 4\|a, 5\|a, 6\|a, 7\|a, 11\|a by rests-on; every other card alone, most joined to G1 by **引入** imports).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 2\|a | §2.1–2.2 | two-country flight-to-safety model | 2\|a | formal; the general framework and the home country H (three assets, four agents, convenience demand); first formal card, kept as head |
| 3\|a | §2.3 | same | 2\|a | formal; the foreign country F of the same two-country model (household, two pricing conditions, government budget) |
| 4\|a | §2.4 | same | 2\|a | formal; the asset-market relations (six indifference conditions) of the same model; rests on 2\|a and 3\|a |
| 5\|a | §3 | same | 2\|a | formal; the equilibrium of the same model (market clearing, aggregate demand, current account) |
| 6\|a | §3.1 | same | 2\|a | formal; the same model log-linearized (13 equations) |
| 7\|a | §4.1 | same | 2\|a | formal; the steady state of the same model, solved recursively from the observed policy rate (calibration) |
| 11\|a | §4.5, §5.1 | same | 2\|a | formal; the baseline shock (AR(1) in θ*) and its impulse responses in the same linear system |
| 12\|a | §5.2 | same | 2\|a | formal; Proposition 1 (regime dependence on the steady-state convenience yield) of the same model |
| 13\|a | §5.3 | same | 2\|a | formal; a variant that changes one assumption (an occasionally binding zero lower bound) and keeps the rest |
| 14\|a | §5.4 | same | 2\|a | formal; a variant that changes one assumption (the central bank sets convenience-asset supply by a rule) |
| 15\|a | §5.5 | same | 2\|a | formal; a variant that changes one assumption (reserve-backed issuance) |
| 17\|a | §5.7 | same | 2\|a | formal; the permanent-flight experiment in the fixed-supply economy of the same model |
| 17\|b | §5.7 | same | 2\|a | formal; the known-duration flight experiment of the same model |
| 19\|a | A.1 | same | 2\|a | formal; derivation of the same model's expenditure minimization (price indices, demands) |
| 19\|b | A.2 | same | 2\|a | formal; derivation of the same model's Calvo pricing and Phillips curve |
| 20\|a | A.3 | same | 2\|a | formal; the same model's deterministic steady state in full |
| 21\|a | B | same | 2\|a | formal; the proofs of Propositions 1–4 of the same model and the kinked-rule threshold |
| 25\|a | C.5 | same | 2\|a | formal; the two-target balance-sheet experiment of the same model |
| 27\|a | C.7 | same | 2\|a | formal; the net resource transfer of the same model (intertemporal budget constraint) |
| 28\|a | C.8 | same | 2\|a | formal; the permanent and finite-duration flight experiments of the same model in full |
| 8\|a | §4.2 | convenience-yield measurement | 8\|a | empirical; measures the Swiss franc convenience yield (1994–2021 three-month rates, about 287 bp at end-2008) as a calibration input; an estimation is never folded into the theory it feeds (as velic2025inflation 7\|a) |
| 16\|a | §5.6 | acute-phase data comparison | 16\|a | empirical; sets six official Swiss statistics for 2008Q3–2009Q2 beside the model's four branches (Table 6): a test of the theory, a model of its own |

Three models. The evidence settled every row: every formal card shares the two countries, the three assets, the convenience demand and the policy rules, and each section solves, calibrates, proves or varies one assumption of that model.

### Fold (executed 2026-11-14, live box)

Method: the per-card fold script of the assenza2026louder pass (`fold.mjs`, with a batch retry), driven in book order by `drive.mjs` (this session's scratchpad), one Card per run: strip **结构**/**引入**, mask every Link to a Model Card or part, park the parts beside the section anchor, retype (facet `第 N 章：…`, emphasis core, level cleared), move the head `alone` to the end of the head's main line, return the parts, restore the body with the rename map. Idea-note Direct chains went to the section anchors (5\|a\|1–10, 6\|a\|1–10, 7\|a\|1–7, 13\|a\|1–7, 15\|a\|1–8, 19\|a\|1–3, 19\|b\|1–6, 20\|a\|1–5, 25\|a\|1–9, 28\|a\|1–8), as did the Idea Side cards 13\|a\|a and 28\|a\|a. Every **引入** line of the folded heads (4\|a, 5\|a, 6\|a, 7\|a, 11\|a and others) named a part that is now 2\|a's own, so none was appended. All 19 runs returned ok.

| Former head | Now | Facet |
|---|---|---|
| 3\|a | 2\|a\|a\|16 | 第 1 章：F 国（§2.3） |
| 4\|a | 2\|a\|a\|17 | 第 2 章：资产市场关系（§2.4） |
| 5\|a | 2\|a\|a\|18 | 第 3 章：均衡（§3） |
| 6\|a | 2\|a\|a\|19 | 第 4 章：对数线性化一般均衡（§3.1） |
| 7\|a | 2\|a\|a\|20 | 第 5 章：稳态识别（§4.1） |
| 11\|a | 2\|a\|a\|21 | 第 6 章：基准经济的外逃冲击（§4.5、§5.1） |
| 12\|a | 2\|a\|a\|22 | 第 7 章：便利收益的作用与区制依赖（§5.2） |
| 13\|a | 2\|a\|a\|23 | 第 8 章：零利率下限（§5.3） |
| 14\|a | 2\|a\|a\|24 | 第 9 章：资产负债表政策（§5.4） |
| 15\|a | 2\|a\|a\|25 | 第 10 章：储备支持型操作（§5.5） |
| 17\|a | 2\|a\|a\|26 | 第 11 章：永久性外逃（§5.7） |
| 17\|b | 2\|a\|a\|27 | 第 12 章：已知持续时间的外逃（§5.7） |
| 19\|a | 2\|a\|a\|28 | 第 13 章：支出最小化（A.1） |
| 19\|b | 2\|a\|a\|29 | 第 14 章：Calvo 定价与菲利普斯曲线（A.2） |
| 20\|a | 2\|a\|a\|30 | 第 15 章：确定性稳态（A.3） |
| 21\|a | 2\|a\|a\|31 | 第 16 章：命题的证明（B） |
| 25\|a | 2\|a\|a\|32 | 第 17 章：两目标的资产负债表实验（C.5） |
| 27\|a | 2\|a\|a\|33 | 第 18 章：净资源转移（C.7） |
| 28\|a | 2\|a\|a\|34 | 第 19 章：永久性与有限期外逃实验（C.8） |

2\|a's own H-country derivations stay the setting's Direct chain (2\|a\|a\|1–15); the restructure makes them chapters.

Gate:

- `model-survey.mjs arrocannarsa2026flight`: 3 Model Cards, exactly the decided heads: 2\|a (formal, 256 parts), 8\|a (empirical, 16), 16\|a (empirical, 40).
- `check-model.mjs 2|a`: 256 members (35 main, 221 side); every former head a part; no closure fault; 3 `forward` faults (chapter 16, 2\|a\|a\|31, names its own parts \|a\|6, \|a\|12, \|a\|20 with `依赖` lines) and 19 `unanchored` chapter cards, loose 2: left to the restructure.
- `check-model.mjs 8|a`: 16 members, loose 1, `self_sufficient` true. `16|a`: 40 members, loose 1, `self_sufficient` true.
- `list_broken_links` (agent): empty.
- Term relink: `{"rounds":1,"ok":true,"cards_changed":5,"links_added":6,"cards_refused":0,"links_repaired":0}`.
