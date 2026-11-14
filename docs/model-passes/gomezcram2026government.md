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

## Stage 3: restructure

Date 2026-11-14, live box, under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; the chapter leads were written by hand). Backup: `D:\zt\backups\zt-before-gomezcram2026government-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Moves ran as four `apply_batch` tree batches (2|a in two batches of 40 and 43 items, because a batch takes at most 64; 3|a 3 items; 20|a 4 items), each previewed, then committed under the plan hash; `links_that_will_break` was empty in every preview. One commit of the second 2|a batch returned no reply; the script previewed again and committed (applied once). Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the equation numbers of the old and new bodies; no display formula and no equation number changed. No `card_locked` error and no `read_view_changed` error occurred. The Literature tree went from 518 to 521 Cards (three new chapter cards: 2\|a\|a\|2, 3\|a\|a\|1, 20\|a\|a\|2). The setting parts kept their structure; the only edits there were chapter numbers in text, the first-meeting column of 22 notation rows (below).

### 2\|a (formal): the intuition spine

```
2|a|a  设定 (setting part as the settings pass left it)
2|a|a|1   第 1 章：文字版本与两条渠道（§1）                 ← 设定, 2|a|a|b
    a 机制 → a|1 结论;  b 例子 (unchanged)
2|a|a|2   第 2 章：抑制楔子与债券定价（§2.1–2.1.1） (new)    ← 2|a|a|c, c|a
    a 结果：楔子抬高债券价格 (6)(7)        (proof a|a 债券一阶条件 (5) → a|a|1 楔子与定价方程)
    b 结果：极限下的类 CAPM 定价核 (8)     (proof b|a ψ→∞ 的效用 → b|a|1 对数定价核;  b|b 注记)
2|a|a|3   第 3 章：货币与财政政策块（§2.3–2.4）             ← 第 2 章, 2|a|a|e
    a 结果：实际预算约束 (17)              (proof a|a 定义 (18) → a|a|1 实际形式)
    b 结果：含楔子的债券价格方程 (19)       (proof b|a)
2|a|a|4   第 4 章：抑制政策、市场出清与均衡（§2.5–2.7）      ← 第 3 章, 2|a|a|f
    a 结果：盈余把资源转给资产持有者 (24)   (proof a|a 消费配置)
    b 定义：竞争性均衡
2|a|a|5   第 5 章：线性化系统与确定性区域（§3.1）            ← 第 4 章, 2|a|a|h
    a 定义：复合参数 (29)
    b 结果：引理 1 四个确定性区域          (proof b|a (26) → b|a|1 (27) → b|a|2 (28))
    c 推导：区域 3 的抑制强度
2|a|a|6   第 6 章：货币配合下的风险溢价（§3.2.3）            ← 第 5 章, 假设 1
    a 前提：均衡解 (31)(32) → a|1 前提：效用的线性化
    b 结果：推论 1.3 风险溢价为正          (proof b|a (A7) → (A8) → (36);  b|b 图)
    c 结果：推论 1.3 抑制降低溢价          (proof c|a (A10) → G 的根)
2|a|a|7   第 7 章：货币不配合时的抑制均衡（§3.3.1）          ← 第 5 章, 假设 2
    a 前提：线性化系统
    b 结果：命题 2 抑制下通胀为零          (proof b|a 通胀为零 → 价格载荷 → 债务载荷 → ω_g* (39);  b|b 图 3)
    c 结果：推论 2.1 财政支撑越弱抑制越强   (proof c|a)
2|a|a|8   第 8 章：线性化均衡与四体制载荷（A.2）             ← 第 7 章, 2|a|a|h|a
    a 结果：载荷通解                        (proof a|a 序列空间递推 → 通胀与价格载荷 → 债务载荷;  a|b 分区图)
    b 结果：体制一;  c 结果：体制二 (proof c|a);  d 结果：体制三 (proof d|a);  e 结果：体制四
2|a|a|9   第 9 章：财政配合体制（§4.1、B.1）                 ← 第 8 章, 假设 3
    a 前提：支出偏离与线性化回报
    b 结果：财政配合的载荷 (A12)(A13)       (proof b|a → b|a|1)
    c 结果：命题 3 抑制抬高债券价格         (proof c|a 三个偏导;  c|b 图 4)
    d 结果：推论 3.1 福利再分配             (proof d|a 福利 1/2 → 2/2 → ω_H 的作用)
2|a|a|10  第 10 章：财政配合体制的风险溢价（B.1.1）          ← 第 9 章
    a 定义：风险溢价
    b 结果：风险溢价为负 (A14)              (proof b|a 回报反应 → 效用弹性 → 闭式;  b|b 图 5)
    c 结果：推论 3.2 溢价的比较静态         (proof c|a)
2|a|a|11  第 11 章：双配合体制的连续统均衡（B.2）            ← 第 8 章, 假设 4
    a 结果：命题 4 均衡连续统 (A16)          (proof a|a 通胀与价格路径 → 债务路径;  a|b 均衡族)
    b 结果：两端的退化
2|a|a|12  第 12 章：抑制的随机持续时间（§4.2、B.3）          ← 第 7 章, 变体五
    a 政策函数形式 (A17)(A18)
    b 结果：状态依赖的载荷 (A19)            (proof b|a 四步载荷;  b|b 证明：(A19) 良定（B.3） → 公共因子 → 特例 (32) → 结果)
    c 结果：命题 5 抑制越不持久效果越弱     (proof c|a 两步比较静态;  c|b 图 6)
    d 结果：推论 5.1 抑制终结的效应         (proof d|a (A20)–(A22);  d|b 图 7)
    e 结果：推论 5.2 终结风险被定价         (proof e|a (A23))
2|a|a|13  第 13 章：非线性方程组（A.1）                      ← 第 4 章, 2|a|a|g
    a 结果：效用与贴现因子 (proof a|a → a|a|1);  b 结果：两种债券的定价;  c 结果：两类消费 (proof c|a)
    d 结果：政府预算与债券回报;  e 结果：政策规则;  f 结果：抑制的两条通道
```

- The base pricing chain (former 2\|a\|a\|2–7) left the main line: a new chapter card `2|a|a|2` (第 2 章) holds the proposition (6)(7) and the CAPM-limit conclusion (8) as results, each with its derivation chain as a Side branch.
- Chapter order follows the book with one exception forced by the premises: A.2 (former 第 10 章, the four-regime loadings) comes right after the active/active chapter and before the fiscal-accommodation chapter, because its regime-3 card rests on 命题 2 (chapter 7) and the fiscal chapter's four proof steps rest on its 体制一 (the five stage-2 forward faults of 第 7 章 → 第 10 章 are gone by this move). B.1.1 and B.2 stay chapters (each states its own results, 推论 3.2 and 命题 4, and B.1.1 is the fiscal counterpart of the §3.2.3 risk-premium chapter); they follow the fiscal chapter. B.3 (former 第 13 章) proves that (A19) is well defined and nests (32): it is now the second proof branch of the result (A19) in chapter 12 (`2|a|a|12|b|b`, facet `证明：(A19) 良定（B.3）`, new title, one opening sentence naming the result, `本模型` → `本卡`). A.1 is the last chapter (it proves no result; it lists the full system).
- Renumbering, old → new: 2→3, 3→4, 4→5, 5→6, 6→7, 10→8, 7→9, 11→10, 12→11, 8→12, 9→13, 13→(B.3 under 12).
- In every chapter the flat chain was cut into results with their proof chains as Side branches. A 前提 or 定义 card that heads a chapter and serves several results stays the chapter's first Side branch (5\|a, 6\|a with 6\|a\|1, 7\|a, 9\|a, 10\|a, 12\|a), as 均衡系统 in assenza 7\|a. The aside cards (图, 注记, 分区图, 均衡族) stay under their result after the proof. Result facets now read `结果：…` (31 Cards; the former `命题`, `结论`, `推论`, `结果`, `体制一`–`体制四`, `均衡条件`, `政策规则` facets; titles unchanged).
- Every chapter card got **直觉** (lead of 20+ characters), **本章结果** (pointers `结果：[[…]]`, `前提：`, `定义：`, `推导：`, `机制：`, `旁注：`) and **承上启下** with two `依赖` lines (the earlier chapter or setting it builds on); the old text follows (`本模型` → `本章`). Chapter facets carry the new numbers. This anchored all 13 chapters.
- Chapter numbers in text, old → new: the 前提 cards `2|a|a|7|a` (第 6/4 章 → 第 7/5 章) and `2|a|a|9|a` (第 7 章 → 第 9 章); the notation card (第 9 章 → 第 13 章, 第 5 章 → 第 6 章); settings `g`, `h`, `i`, `j`, `k`, `l`, `m` (variant five: 第 13 章 → 第 12 章).
- With results now ahead of their proofs, `check-settings` found 12 symbols first met in a result card. The notation rows now name those cards; three result cards got one defining clause ($M_{t+1}$ in 2\|a; $rp^{gr}$ in 6\|b; $r_{\varepsilon,k}^{gr}$ in 9\|c); the others already define theirs ($m_{t+1}$, $r_{At+1}$ in 2\|b; $B_g$, $Q_g$, $\Pi_g$ in the (A19) display of 12\|b; $u_{A,\varepsilon}$ in 10\|a; the two $rp^{gr,i}$ in 12\|e). The $B$ row now points to 2\|a\|a\|2\|a, because the check reads $B$ in $Q_tB_{it}$ (as stage 2 open item 5). `cite`: 第 6 章's 本章结果 line and the B.3 opening sentence now link 假设 1 and 假设 5 in place of the bare names.
- The Model Card: new title (基准模型：金融抑制经定价楔子与财政再分配改变政府融资成本，效果取决于货币—财政体制), the question widened to the regimes, one model-wide sentence added to 概要 (the old 概要 kept), **结构** rebuilt from the tree, **分部** (设定与直觉 from 2\|a\|a; 基准模型 from 第 2 章; 线性化与货币主导 from 第 5 章; 财政配合 from 第 9 章; 随机持续时间与非线性系统 from 第 12 章).

Network (`check-network.mjs`): 设定 → {1, 2, 3, 4, 7, 13}; 4 → 5 → 6; 7 → {8, 12}; 8 → {9, 11}; 9 → 10.

### 3\|a (formal)

```
3|a|a   设定 (unchanged)
3|a|a|1  第 1 章：监管约束与楔子的闭式（§2.2.1–2.2.2） (new)   ← 3|a|a|b, 3|a|a|c
    a 结果：监管约束映射为楔子   (proof a|a 边际监管价值 (12) → a|a|1 楔子取闭式)
```

The two derivations and the proposition left the main line under a new chapter card. The notation rows of $\mu_t$ and $\Omega_t$ now name the result card, which now writes `抑制楔子 $\Omega_t$ 的定义 (6)`. **结构** rebuilt. The eight legacy shares and the 共用前提 block stay for the import stage.

### 20\|a (formal)

```
20|a|a  设定 (unchanged)
20|a|a|1  第 1 章：文字版本（§2.2.3）                       ← 20|a|a, 20|a|a|b
    a 机制 → a|1 结论 (unchanged)
20|a|a|2  第 2 章：杠杆约束与楔子（附录 C.1） (new)          ← 20|a|a|c, 20|a|a|c|a
    a 结果：楔子的决定式 (A28)        (proof a|a (A27) → a|a|1 对数线性化)
    b 结果：量化宽松抬高楔子
```

Chapter 1 got **直觉**/**本章结果**/**承上启下** (anchored on the setting card and the overview). Eight notation rows ($v_{t+1}$, $n_{t+1}$, $r_{t+1}^D$, $\Omega^{ss}$, $\bar\mu^{GK}$, $\chi_{V/N}$, $\chi_D$, $\chi_\mu$) now name the result card; it got one clause defining $\Omega^{ss}$. **结构** rebuilt.

### Gate

- `check-model.mjs 2|a`: 144 members (14 main, 130 side), loose 0, no closure, forward or unanchored fault, `self_sufficient` true (was false). `3|a`: 16 members (2 main, 6 side, 8 shared), loose 1, `self_sufficient` true. `20|a`: 16 members (3 main, 10 side, 2 shared, 1 off), loose 1, `self_sufficient` true. The two loose Links are the Model Cards' cross-model **关联** lines (3\|a → 20\|a\|a\|1, 20\|a → 2\|a\|a\|1), as after stage 2.
- `check-network.mjs`: `network: clean` for all three (advice only: chapter 1 of 2\|a and of 20\|a has no `结果：` card, right for the verbal chapter).
- `check-premises.mjs`: 20\|a `premises: none used before their card`. 2\|a 25 hits, all pointers: 22 are the setting-part **直觉**/**说明**/`依赖` lines stage 2 listed (c\|a, d, e, e\|d, e\|e, e\|f, h, h\|a, i, j, l naming (6), (17), (19), (21)–(24), (26)–(29), (31), (32)); 3 are the definition (29) `2|a|a|5|a`, whose **直觉** lines say which of (26)–(28) the composite parameters enter. 3\|a 2 hits: `2|a|a|c|a` (stage-2 pointer to (6)) and `2|a|a|2|a|a|1`, the proof step of the result that states (6); it derives (6) itself.
- `check-settings.mjs`: `settings: all in the setting part, as a dependency tree` for all three (2\|a advice: 12 settings under the setting card).
- `list_broken_links` (agent): `{"broken":[]}`. Literature tree 518 → 521 Cards.
- Term relink: `{"ok":true,"links_repaired":0,"links_added":92,"applied":true,"rounds":1,"cards_changed":21,"book":"gomezcram2026government","cards_refused":0}`. All checks above were rerun after it and stay as listed.

### Open

1. Step 3 card text was not redrafted: result cards keep their digest body (no `**为什么**`/`**证明**` sections), derivation cards keep their digest format, chapter cards keep their old 问题/概要/关联 text below the new sections. The chapter-1 cards of 2\|a and 20\|a keep the facets 机制/结论.
2. Chapter 13 (A.1) restates the ψ→∞ utility and the SDF derivations of chapter 2 (`13|a|a`, `13|a|a|1` against `2|b|a`, `2|b|a|1`); the import stage may share them.
3. `2|a|a|5|c` (推导：区域 3 的抑制强度) stands alone under chapter 5; it ends in no result card of its own. Inside the B.3 proof branch, `2|a|a|12|b|b|a|2` keeps its facet `结果`.
4. 3\|a: the legacy shares, the 共用前提 block and the Links to 2\|a's chapter 4 for the repression rule (stage 2 open item 6) are the import stage's.
5. The setting card of 2\|a still carries twelve settings (advice `wide`, stage 2 open item 2).
6. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.

## Stage 4: imports and explanations

Date 2026-11-14, live box, under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent; one DeepSeek job, for the explanation refresh). A first attempt stopped before any write: the `VACUUM INTO` failed with `database or disk is full` (drive D had 0 GB free); the partial backup file was removed and the owner freed space (about 20 GB). Backup: `D:\zt\backups\zt-before-gomezcram2026government-imports.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Text ran as `edit_card` calls under `-Allow` (gomezcram2026government Cards only, 6 Cards written). Before each text write a script compared the display formulas, the equation numbers and the count of backslashes of the old and new bodies; no display formula, equation number or estimate changed, and every written Card was read back with its TeX intact. No `card_locked`, no `read_view_changed` and no failed batch occurred. Order: 2\|a, then 3\|a and 20\|a.

### 2\|a (formal, baseline)

- No import. The baseline takes the wedge Ω as a primitive: the portfolio constraint (3) (`2|a|a|c|a`) and the repression rule (20) (`2|a|a|e|e`) are its own settings, and no part rests on 3\|a or 20\|a. The two micro-foundations explain where Ω comes from; a micro-foundation is borrowed by the model it founds, not the reverse, so 2\|a imports nothing from them (the fold decision, stage 1). `check` exit 0, `plan` `nothing to import`, tolerated 0, false shares 0.
- Copy advice judged by hand: `2|a|a|3` (第 3 章, the policy block) ~ `20|a|a|1` (20\|a's verbal QE chapter). Not a copy: the policy block states the Taylor rule (13), the tax rule (14), the spending process (15), the budget constraint (16)–(18) and the price equation (19); its only tie to the QE chapter is the comparison line `对应 [[20|a|a|1]]：量化宽松把中介约束写成时变抑制规则` in **关联**. It stays a comparison; the advice remains in `check` output.
- Stage-3 open item 2 (chapter 13 restates the ψ→∞ utility and the SDF derivation of chapter 2) is inside one model, so no import applies (`import_target` forbids importing from the model's own range); left open.
- 2\|a now lends `2|a|a|2|a` (with its closure) to 3\|a (`lent`).

### 3\|a (formal, micro-foundation of the wedge)

- Kept: `整块 [[gomezcram2026government/2|a|a|2|a]]：约束 (3)、乘子与楔子 (6)、定价方程 (7)` (written in the fold/settings passes from the old （共用） lines). Its closure brings the setting card 2\|a\|a, the budget constraint 2\|a\|a\|c and the constraint (3) 2\|a\|a\|c\|a, and the proof chain 2\|a\|a\|2\|a\|a → \|a\|1 (6 shared members). The setting card rests on the result; the closed-form step `3|a|a|1|a|a|1` rests on the wedge definition (6) in `2|a|a|2|a|a|1`.
- The two remaining legacy shares were false shares (comparisons on the Model Card; no part rests on either), so `plan`'s two `整块` proposals were not applied:
  - `2|a|a|4` (第 4 章, equilibrium): the Model Card named it in **概要** and **关联** for the repression rule. Now `并映射到基准模型的抑制规则 (20)` and `对应 [[2|a]] 的约束 (3)、楔子 (6) 与抑制规则 (20)`. The result card `3|a|a|1|a` pointed twice to chapter 4 for the rule; it now names the rule's setting card: `这个时变楔子映射到基准模型的抑制规则 (20)，见 [[2|a|a|e|e]]。抑制规则本身由那张设定卡给出，本卡不重复。` (stage 2 open item 6, stage 3 open item 4).
  - `20|a|a|1` (20\|a's verbal chapter): a comparison of one micro-foundation with another. **关联** now reads `对应 [[20|a]]（第 1 章文字版本与附录 C.1）的量化宽松中介抵押约束`.
- Tolerated 4 → 0 (the four were premises of the two false shares), false shares 0, legacy shares 2 → 0. Symbols (step 6): 3\|a has its own notation card (`3|a|a|a`, 14 rows); `check-settings` finds no symbol of the imported block uncovered, so no `沿用` line was needed.

### 20\|a (formal, QE intermediary model)

- No import. The model defines Ω itself in its derivation (`20|a|a|2|a|a|1` step 1, the ratio of the bond price to its discounted payoff, noted as equivalent to (7)) and rests on no part of 2\|a; the Model Card says `抑制楔子的定义式 (6) 与定价式 (7) 在 [[2|a]] 中给出，本模型不重述` as a plain reference.
- Two false shares removed from **关联**: `对照 [[2|a|a|1]]：该模型把抑制楔子当政策参数……` and `参照 [[2|a|a|3]]：该政策块给出模型闭合用的政策规则` were folded into the existing whole-model line: `对照 [[2|a]]：抑制楔子与政府债券定价由该模型给出，它把楔子当政策参数（第 1 章），用政策块（第 3 章）的规则闭合模型；本模型只补金融中介的摩擦，给出楔子的来源。` Tolerated 4 → 0, false shares 2 → 0.

### Cross-book blocks

`plan` proposed no cross-book line for any head, and no part of the three models rests on another book's Card. The cross-book Links (assenza2026louder/5\|b\|e\|c\|1 on the policy block, bergin2026exchange/5\|f and ozhan2026monetary/27\|a on chapter 4, atkinson2015lectures/18\|c\|c\|1 on 20\|a's chapter 1) are comparisons (`对照`, `对应`, `类比`), kept as they are.

### Explanation refresh (step 8)

- `explain.mjs plan`: 2\|a 4 Derivation parts in 1 job; 3\|a and 20\|a `nothing to explain`.
- One DeepSeek conversation (`opencode-go/deepseek-v4.1-flash`, effort high, the box's drive model; OpenCode Go account Primary, 3 requests, 91 s, 1 turn, context 57k of 1M; tools: 1 pwsh, 1 write, only `out/explain-01.json` written): answer `{"done":4,"blocked":0}`. Cost: OpenCode Go subscription, the Harness ledger books 0 CNY for the source (the 2026-11-14 opencode-go day line went from 1,123 to 1,162 calls during the job but is shared with the IMEN1 queue, so no per-job token split). Conversation adc28d26 deleted, workspace removed.
- `apply`: `applied 4, refused 0, blocked 0, follow-up jobs 0`. `finish`: `explained 4 part(s); Term links changed 0 Card(s), second plan 0; left 0; refused 0` (exit 0); parts `2|a|a|11|a|a`, `11|a|a|1` (B.2 continuum paths) and `2|a|a|12|b|b|a`, `12|b|b|a|1` (B.3: (A19) well defined).
- The refreshed text named `假设 4` (`11|a|a|1` 解读) and `假设 5` (`12|b|b|a` 思路 and one step's 说明) without the Link; `check-settings` reported two `cite` faults. The bare names were replaced by `[[2|a|a|l]]` and `[[2|a|a|m|a]]` (text only, backslash count unchanged); the faults are gone, and a later `explain.mjs plan` and `finish` on 2\|a find nothing.

### Gate

- `imports.mjs check`: exit 0 on 2\|a, 3\|a, 20\|a; `plan`: `nothing to import` on all three (2\|a keeps the copy advice above).
- `get_model`: `self_sufficient: true` on all three; tolerated 0 and false shares 0 everywhere; 3\|a imports `整块 2|a|a|2|a` (6 shared members); 2\|a lends it.
- `explain.mjs finish`: exit 0 on all three (`explained 0 part(s); … left 0; refused 0` on the rerun).
- `check-model.mjs`: 2\|a 144 members `{"main":14,"side":130}`, loose 0, self_sufficient true; 3\|a 14 `{"shared":6,"main":2,"side":6}`, loose 0 (was 1), self_sufficient true; 20\|a 14 `{"main":3,"side":10,"off":1}`, loose 0 (was 1), self_sufficient true.
- `check-network.mjs`: `network: clean` on all three (advice only: chapter 1 of 2\|a and 20\|a has no `结果：` card, as at stage 3).
- `check-settings.mjs`: `settings: all in the setting part, as a dependency tree` on all three (advice only: 2\|a 12 settings under the setting card; `uses an assumption` lines on 2\|a\|a\|12\|d\|a, 2\|a\|a\|13\|c\|a and 20\|a\|a\|2).
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"applied":false,"ok":true,"links_repaired":0,"links_added":0,"rounds":0,"cards_changed":0,"cards_refused":0,"book":"gomezcram2026government"}`. All checks above were rerun after it and stay as listed.

### Open

1. The copy advice `2|a|a|3` ~ `20|a|a|1` stays in `check` output; judged a comparison, not a copy.
2. Chapter 13 (A.1) of 2\|a still restates the ψ→∞ utility and SDF derivation of chapter 2 (stage 3 open item 2); inside one model, so it is a matter for a later restructure, not an import.
3. Stage-3 open items 1, 3 and 5 (result and derivation text not redrafted; `2|a|a|5|c` alone; twelve settings under the setting card) stay.
4. The explanation job wrote assumption names without their Links; fixed by hand here. The explain prompt could ask for `[[…]]` in place of a bare `假设 N`.
5. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.

## Stage 5: illustrate

Date 2026-11-14, live box (owner's request), under `zt-model-illustrate`, done in one side session by scripts. No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-gomezcram2026government-illustrate.sqlite3` (`VACUUM INTO`, 397 MB, `PRAGMA integrity_check` ok; drive D had 66 GB free). All three models are formal, so all three were illustrated. Modules: `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\gomezcram2026government-2_a.mjs`, `…\gomezcram2026government-3_a.mjs` and `…\gomezcram2026government-20_a.mjs` (uncommitted, under `~/.claude`). Before the live write, each figure was rendered locally from the module (a copy of `verify.mjs` that reads the figures from the module, session scratchpad `local.mjs`) and the PNGs were inspected; faults fixed there: `t−1` subscripts that `m()` cannot read without braces, `<` and `>` inside caption TeX read as HTML tags (now `\lt`, `\gt`), a sub-plus-superscript `V^L_At`, overlapping curve labels, a clipped number-line label, a dashed feedback path through two boxes. `apply.mjs --write` then wrote 27 + 4 + 5 Cards, and a second write reworded two 直觉 lines (2\|a\|a\|c, 2\|a\|a\|i) that zt-check flagged R21 (advisory, unnamed referent). No display formula, equation number or text outside the chart and the **直觉** section changed (apply guard). Every Model Card figure has three parts (`part()`, top-level `data-part`): 一 · 博弈与时序, 二 · 因果结构, 三 · 模型研究的变化.

### Figures

- Model Card 2\|a (**图**, before **结构**): (一) the order of one period: the spending shock and (15); the government and the central bank by rules (13), (14), (20), (21), (16); the asset holders' problem ((1) under (2) and (3), the only optimizer); the hand-to-mouth agents (9); clearing (22)(23); the solution order by chapter (2 → 3 → 4 → 5 → 6, 7, 9–11 → 12 → 13; 1 verbal). (二) four rows: shock, policy coefficients $\rho_\pi,\delta_b$, $\omega_g$, $\omega_H$ → surplus, rate, wedge → real debt (17), bond price (19), the two consumptions (24) → inflation, risk premium and funding cost, welfare redistribution; dashed feedbacks (debt → taxes, inflation → rate, consumption via the SDF → price). (三) twelve panels: 第 2 章 wedge raises the price; 第 4 章 redistribution; 第 5 章 four regions; 第 6 章 risk premium; 第 7 章 (two panels); 第 9 章 (two panels); 第 10 章 negative premium; 第 11 章 continuum; 第 12 章 duration; 第 13 章 nonlinear system. Each box opens its Card.
- 2\|a setting cards (22), at the end of **说明**: a (unit mass split $\zeta$ / $1-\zeta$, the endowment, (1) as current consumption ⊕ certainty equivalent, $\gamma$ and $\psi$ separate); b (two agent types and the government, the three tools, each opening its rule); c (uses against sources of (2), the multiplier $\lambda_t$); c\|a (a feasible set in bond market value × other assets, 示意, the shadow value $\mu_t$); d (the hand-to-mouth income, minus the mandated purchase, to consumption); e (central bank, fiscal authority, third tool; the perpetuity split into maturing $\delta$ and rolled $1-\delta$); e\|a (the rule line through $(\pi^\star,i^\star)$, two slopes 示意); e\|b (the tax line through $\tau^\star$, the number line of the lower bound on $\delta_b$); e\|c (one shock decaying at $\rho_g$, 示意); e\|d (the geometric payments of one perpetuity, (16) as issue revenue − maturing payment = deficit); e\|e (log Ω against $g_t$, two strengths); e\|f (the face split $\omega_H$ / rest, the number line $[0,1)$); f ((22) and (23) as weighted sums); g (the $t-1$ / $t$ time line, the 11 endogenous variables by owner); h (a curve and its tangent at the steady state, 示意; how deviations are written); h\|a (a loading profile $x_{\varepsilon,k}$, 示意; $x_t$ as the weighted sum); i, j, k, l (the $(\delta_b,\rho_\pi)$ plane with the admitted quadrant shaded, named by content, never by the assumption name; i also the number line $0\le\omega_g<\rho_g\Psi$); m (the two-state Markov chain $r$ ⇄ $n$ with the persistence probabilities); m\|a (the number line $1/2<\rho_\omega^i\le1$).
- Model Card 3\|a: (一) government sets the regulation → competitive intermediaries pass it on → asset holders choose under (2) and (10) or (11) → the multiplier enters the imported pricing equation; solution order 第 1 章 ((12), then the wedge (6)). (二) fiscal position → $\tilde{\mathcal B}_t$ and $\chi_L,\kappa_L$ → marginal regulatory value 1 (12) and the multiplier → the wedge $1/(1-\mu_t)$ → bond price and the time-varying wedge mapped to rule (20). (三) three panels (all 第 1 章).
- 3\|a setting cards (3): a (government → intermediary → asset holders, the three parameters); b ((10) as two terms, the number line of admitted bond value above $\tilde{\mathcal B}_t$); c ((11) as a stacked bar of weighted liquid assets against the required cover, 示意, number lines $\chi_L\in[0,1]$, $\kappa_L>0$). The imported block (2\|a\|a\|2\|a with its closure) is 2\|a's and was skipped.
- Model Card 20\|a: (一) central bank buys $\psi_{g,t}$ → intermediary chooses $B_t^A$ under the leverage constraint → households hold equity and deposits → returns realize; solution order 第 2 章 (A27) → (A28). (二) purchases, the agency friction $\theta_t^{GK}$, the deposit return → multiplier $\mu_t^{GK}$ and the shadow value of net worth → wedge (A28) → bond price and the time-varying repression rule, with the signs of (A28) on the arrows. (三) four panels (第 2 章 ×3, 第 1 章).
- 20\|a setting cards (4): a (balance sheet (A24) as two bars, flow of funds); b (households → constrained intermediary → bond market; what is known at $t$ and at $t+1$); c (continuation value against divertable assets, the admitted holdings left of the crossing, 示意); c\|a (given / chosen / objective / two constraints, the two blocks of next-period net worth and of the continuation value).

### 直觉

The **直觉** section was written or rewritten on all 29 setting cards (2\|a: a, b, c, c\|a, d, e, e\|a–e\|f, f, g, h, h\|a, i, j, k, l, m, m\|a; 3\|a: a, b, c; 20\|a: a, b, c, c\|a) and on the four 定义 parts of 2\|a (3\|a\|a 事后实际回报, 4\|b 竞争性均衡, 5\|a 复合参数, 10\|a 风险溢价). 3\|a\|a\|b, 3\|a\|a\|c, 20\|a\|a\|c\|a and 2\|a\|a\|10\|a had none. The digest's sections carried Links (2\|a\|a\|b, g), later equation numbers ((17), (19), (21)–(24), (26)–(28), (31), (32), (A10)) and three `（补：书中未写）` marks; the new sections name chapters instead, with no Link, no result, and no assumption name except the card's own. In setting cards they use $TeX$ only for symbols the card or an earlier setting introduces (check-settings `first`). `check-premises` hits on 2\|a fell from 25 to 10.

### Chart fixes outside the setting part

`verify.mjs --model` also lints the digest's own charts. Five failed in 2\|a: 5\|b (raw `δ_b`, `ρ_π`), 13\|f (raw `ω_H`), 8\|a\|b (`rho_pi`, `Phi`, `pi`, `omega` in ASCII; no axis labels), 10\|b\|b (two plots without axis labels) and 11\|a\|b (raw `pi*`, no axis labels). Only their chart fences changed (session scratchpad `fixcharts.mjs`, guard: text outside the fence identical): subscripts became tspans, the ASCII names Greek, and the axis texts labels with `data-axis` (利率对通胀的反应 ρ_π / 税收对债务的反应 δ_b; 财政支撑 Φ; 抑制强度 ω_g, 强制持有 ω_H / 风险溢价 rp; 通胀载荷 π_ε^* / 载荷). No region, curve or number changed.

### Gate

All lines below are the reruns after the Term relink.

- `verify.mjs --model gomezcram2026government/2|a` (light and dark): `28 chart(s) in 145 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets.
- `verify.mjs --model gomezcram2026government/3|a`: `7 chart(s) in 15 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets.
- `verify.mjs --model gomezcram2026government/20|a`: `5 chart(s) in 15 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets. The PNGs, one per part, were inspected.
- `apply.mjs` rerun: `same` for all 27 + 4 + 5 Cards; `fixcharts.mjs` rerun: `same` for all 5.
- `check-settings.mjs`: `settings: all in the setting part, as a dependency tree` on all three (2\|a advice unchanged: 12 settings under the setting card, the `uses an assumption` lines).
- `check-model.mjs`: 2\|a `members 144 {"main":14,"side":130}`, `relations {"refers":138,"rests-on":247,"corresponds":15,"illustrates":9,"contrasts":13}; loose 0; self_sufficient true`; 3\|a `members 14 {"shared":6,"main":2,"side":6}`, loose 0, self_sufficient true; 20\|a `members 14 {"main":3,"side":10,"off":1}`, loose 0, self_sufficient true.
- `check-network.mjs`: `network: clean` on all three.
- `check-premises.mjs`: 2\|a 10 hits (was 25), all existing **说明**/**起点** pointers (c\|a (6), e (19), e\|e (6), h (26)–(29), i (29), j (26)(28)); 3\|a 2 hits (unchanged, stage 4); 20\|a `premises: none used before their card`.
- `zt-check --cards` on the 41 written Cards (run in batches of 7, retried on `changing read view`): `faults: 4`, all in existing text: 2\|a 概要 (R2, R3), 2\|a\|a\|m **起点** (R3), 20\|a **关联** (R3, stage 4). None is in a 直觉 section or a chart. The two R21 advisories on new 直觉 lines were reworded away.
- `list_broken_links` (agent): `{"broken":[]}`, before and after the relink.
- Term relink: the first runs failed on a changing archive revision (other sessions writing); the run that applied the new Term links (visible in the 直觉 sections, e.g. 财政扩张 in 2\|a\|a\|e\|e, 资产持有者 in 3\|a\|a\|b) was inside a retry loop and its JSON line was not kept. The next run: `{"links_repaired":0,"links_added":0,"cards_changed":0,"book":"gomezcram2026government","rounds":0,"cards_refused":0,"ok":true,"applied":false}`; `-PlanOnly`: `{"book":"gomezcram2026government","ok":true,"links_repaired":0,"cards_changed":0,"links_added":0,"cards_refused":0,"rounds":0,"applied":false}`. All checks above were rerun after it.

### Open

1. Stage 7 (literature organize) will renumber addresses. zt 0.63+ rewrites chart `data-zt` targets with the moves, but each module's `head` (and the 3\|a module's `B()` prefix `gomezcram2026government/2|a|a`, which names imported 2\|a parts) must be changed to the new addresses, then `apply.mjs --write` rerun on each new head and `verify.mjs --model` must show 0 dead targets.
2. The four zt-check faults (2\|a 概要 R2/R3, 2\|a\|a\|m R3, 20\|a R3) predate this pass and were not changed.
3. The 10 `check-premises` hits on 2\|a and 2 on 3\|a are existing pointers in **说明**/**起点**; none is in a figure or a 直觉 section.
4. The digest chart 10\|b\|b keeps its two small 200-wide panels (font 9); only labels were fixed. The causal figure of 2\|a has 13 boxes and three feedbacks and reads small at 640 px; the chart sheet shows it alone as part 二.
5. The relink JSON of the applying run was lost (Gate); the follow-up plan is empty, so no Card is left unlinked.
6. No `card_locked`, no failed write. `read_view_changed`-type refusals (zt-check "changing read view", relink "archive changed while reading") were retried. Other sessions wrote other books on the box throughout. Nothing committed (stage brief).
