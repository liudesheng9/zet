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

## Stage 2: settings (strict)

Date 2026-11-14, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-gomezcram2026government-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The three notation cards were created first by `create_successor`. The moves and the four deletes ran as two `apply_batch` tree batches (preview, then commit under the plan hash; `links_that_will_break` empty in both previews). Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the equation numbers (`\tag{…}` and `\quad (N)`) of the old and new bodies; no display formula and no equation number changed, and no display formula was added. No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 519 Cards to 518: three notation cards were created and four restating setting cards were deleted.

### 2\|a (formal)

Every folded chapter's setting card left its chapter or was merged. A setting moved by `move_span` alone, so the derivation chain that stood on its Direct line rose into the chapter's Side slot `a`; every chapter now opens on its own derivations. The budget constraint (2) and the constraint (3) left the main line, so the chapters renumbered: 第 2 章 is now 2\|a\|a\|8, … 第 13 章 2\|a\|a\|19.

```
2|a|a   设定：两类家庭、禀赋与 Epstein-Zin 偏好 (1)                     原始设定
├ a  符号表与读法 (new; blocks: 家庭与偏好, 预算与组合约束, 手到口主体, 政策块, 线性化/单一冲击/体制, 随机持续时间, 推导辅助符号, 术语)
├ b  设定：概述（§1） (from 1|a)                                         原始设定
├ c  设定：预算约束 (2) (from 2|a|a|2, main line)                         ← 2|a|a
│    c|a 假设：抑制约束 (3) (from 2|a|a|3, main line)                     ← c
├ d  设定：手到口主体 (9) (from 11|a|1)                                   ← 2|a|a, c
├ e  设定：政策块 (from 10|a; 11|a merged into 2|a|a)                     ← 2|a|a, c
│    e|a 货币规则 (13) (10|a|1);  e|b 税收规则 (14) (10|a|2);  e|c 支出过程 (15) (10|a|3)
│    e|d 永续债与预算约束 (16) (10|a|4)                                   ← e, e|b, e|c
│    e|e 抑制规则 (20) (11|a|2)                                           ← e, e|c, c|a
│    e|f 强制持有规则 (21) (11|a|3)                                       ← e, d
├ f  设定：出清与资源约束 (22)(23) (from 11|a|4)                          ← c, d, e|f
├ g  设定：时序与内生变量 (from 17|a)                                     ← e, f
├ h  设定：一阶线性化 (from 12|a; 20|a merged in)                         ← e, f
│    h|a 设定：单一冲击与载荷记号 (from 18|a; 13|a merged in)              ← h, e|c
├ i  假设 1 (30) (from 13|a|1)                                            变体一 ← h
├ j  假设 2 (37) (from 14|a|1)                                            变体二，与 i 并列 ← h
├ k  假设 3 (A11) (from 15|a|1)                                           变体三，与 i、j 并列 ← h
├ l  假设 4 (A15) (from 20|a|1)                                           变体四，与 i、j、k 并列 ← h
└ m  设定：变体五：随机持续时间 (from 16|a; 21|a merged in)              变体五 ← e|e, i
     m|a 假设 5 (from 16|a|1)                                             ← m
```

- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**. The thirteen symbol tables and the term tables of the folded setting cards went into the notation card (108 rows), which covers every symbol of every setting and the auxiliary symbols the chapter derivations introduce with 记 ($x_t$, $b_{\varepsilon}$, $b_{g}^{m}$, $B$, $a$, $X$, $\pi_{\varepsilon}$, $\varepsilon$, $\Theta$). Each row names the card where a reader meets the symbol first, as the check reads it.
- Deleted `into` the card each restates (Links re-pointed): 11\|a (chapter 3: the two agent types and three instruments) into the setting card 2\|a\|a; 13\|a (chapter 5: agents, shock timing, rational expectations) into the shock setting `h|a`; 20\|a (chapter 12: agents and rules) into the linearization setting `h`; 21\|a (chapter 13: the two states) into the variant `m`. Their one new statement each moved first: the initial debt $B_{-1}$ into `e|d`; the shock variance $\sigma_\varepsilon^2$, the start-of-period timing and rational expectations into `h|a`; the role of $\mathbb{E}_{t-1}$ in (27) into `h`; the timing of the state and the shock and the remark on $\omega_g^r$ into `m`.
- Retyped and kept in their chapters, because each holds display formulas found in no other card: the chapter-6 setting card (former 14\|a, now `2|a|a|12|a`) states the linearized system (26)–(29), which chapter 4 derives; it is now `前提：线性化系统` at the head of chapter 6. The chapter-7 setting card (former 15\|a, now `2|a|a|13|a`) states the AR(1) form of $g_t$ and the linearized real return; it is now `前提：支出偏离与线性化回报`. The chapter-11 setting card (former 19\|a, now `2|a|a|17|a`) defines the risk premium $rp^{gr}$; it is now `定义：风险溢价`, and it took the verbal definition of the risk premium from 13\|a. The restated agent and timing paragraphs and the tables of these three cards were dropped. The composite parameters (29) (`2|a|a|10|a`, 定义) stay in chapter 4, as derived definitions stayed in the assenza and bergin passes.
- Statements moved and pointers: the policy block and the regime assumptions no longer rest on chapter cards (the former `依赖` lines to the bond-pricing proposition, the price equation (19), the equilibrium definition and Lemma 1 became pointers in **说明**); the repression rule (20) rests on the constraint (3) and points to the derivation of the wedge (6); the sentence on $Q_{t+1}$ and $\Pi_{t+1}$ in (19) is a pointer in the policy block. The four regime assumptions and the stochastic-duration variant each open with a `变体` line.
- Facets now name the assumptions (`假设 1` … `假设 5`). The 24 chapter cards that named an assumption without its Link now link its card in that place; three of them (chapter 5, its first premise card, and the last result of chapter 13) opened a line with `假设 N` and were reported as `stated`; the Link now opens those lines.
- Definitions added at first use: $\mathbb{E}_t$ in the setting card, $i^{\star}$ in `e|a`, $B_{At}$ and $B_{Ht}$ in `f`.

### 3\|a (formal)

```
3|a|a   设定：完全竞争的中介把监管约束直接传给资产持有者，政府设定监管参数   ← 2|a|a|4 (shared)
├ a  符号表与读法 (new; 14 rows)
├ b  设定：强制持有约束 (10) (from 3|a|a|1, formerly facet 约束：强制持有)        ← 3|a|a
└ c  设定：流动性覆盖率约束 (11) (from 3|a|a|2, formerly facet 约束：流动性覆盖率) ← 3|a|a
```

The two regulatory constraints are the model's primitives, so they became settings; the derivations (12) and the closed-form wedge and the proposition now stand on the main line as 3\|a\|a\|1–3. The setting card is in the card format; its symbol and term tables went into the notation card. The eight legacy shares of 2\|a parts and the **引入** line were left for the import stage.

### 20\|a (formal)

```
20|a|a  设定：中介、资产持有者、政府与央行，与资产负债表 (A24)   原始设定
├ a  符号表与读法 (new; 39 rows)
├ b  设定：概述（§2.2.3） (from 20|a|a|1|a)                       原始设定
└ c  假设：杠杆约束 (A25) (from 20|a|a|2, main line)              ← 20|a|a
     c|a 设定：最优化问题 (A26) (from 20|a|a|3, main line)         ← 20|a|a, c
```

The intermediary's problem (A26) states its objective and its two constraints, so it became a setting under the leverage constraint; the derivations (A27), (A28), the proposition and the conclusion now stand on the main line as 20\|a\|a\|2–5. The verbal chapter's mechanism and conclusion rose into 20\|a\|a\|1\|a and \|a\|1. The pointer to the definitions (6) and (7) of 2\|a moved to **说明**. The notation card has a row `$A$` because the equations of this model are numbered `\quad (A24)` to `(A26)` inside the display, and the check reads the A as a letter; the row says it is an equation prefix.

The **结构** sections of the three Model Cards were rebuilt from the build order (2\|a: the setting part and the main line; 3\|a and 20\|a: every own part).

### Gate

- `check-settings.mjs gomezcram2026government/2|a`: `settings: all in the setting part, as a dependency tree` (advice only: the setting card has 12 settings filed under it).
- `check-settings.mjs gomezcram2026government/3|a`: `settings: all in the setting part, as a dependency tree`.
- `check-settings.mjs gomezcram2026government/20|a`: `settings: all in the setting part, as a dependency tree`.
- `check-model.mjs 2|a`: 143 members (20 main, 123 side), loose 0, no closure or forward fault; `unanchored` on the 13 chapter cards (2\|a\|a\|1, 8–19), as after stage 1.
- `check-model.mjs 3|a`: 15 members (4 main, 3 side, 8 shared), loose 1, `self_sufficient` true.
- `check-model.mjs 20|a`: 15 members (6 main, 6 side, 2 shared, 1 off), loose 1, `unanchored` on chapter 20\|a\|a\|1, as after stage 1.
- `list_broken_links` (agent): empty, before and after the relink.
- Term relink: `{"rounds":1,"book":"gomezcram2026government","ok":true,"applied":true,"cards_changed":27,"cards_refused":0,"links_added":80,"links_repaired":0}`. The three settings checks and `list_broken_links` were rerun after it and stay clean.

### Open

1. 2\|a: the 13 chapter cards and 20\|a's chapter 1 are unanchored, and 2\|a's own derivation chain (Lagrangian, wedge, pricing, CAPM limit; 2\|a\|a\|2–7) still stands on the main line between chapter 1 and chapter 2. The restructure builds the spine.
2. The setting card of 2\|a carries twelve settings (advice `wide`). The four regime assumptions and the stochastic-duration variant could be grouped under one block card of the policy regimes; the pass kept them as top-level variants, as the task asked, without a new card.
3. The regime assumptions use $\Theta$, $\Psi$, $\Phi$, whose definition (29) stays in chapter 4 (`2|a|a|10|a`); the settings point to it. The restructure or the import stage may decide to make (29) a setting.
4. `check-premises.mjs 2|a` lists pointers from settings to later equations ((6), (17), (19), (21)–(24), (26)–(29), (31), (32)); all are pointers in **直觉** or **说明**, not uses.
5. The check reads some symbols first in a card that only carries them in a sub- or superscript or in a function argument ($r$ in $R^{gr}_{t+1}$ in the policy block, $G(\delta_b)$ in the spending process, $x_t$ in the loading sum); the rows point where the check says.
6. 3\|a's head and its proposition still link chapter 3 of 2\|a (now 2\|a\|a\|9) for the repression rule; the rule itself is now the setting 2\|a\|a\|e\|e. The import stage settles these Links with the shares.
