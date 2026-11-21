# Model passes: bedford2026learning

Bedford, Moers and Timmermans (2026), *Learning and Frictions in Manager-Firm Matching* (Scholar Archive Work 30032, bilingual file 2408). Digest: zt-econ-digest 6.12.10 on OpenCode Go, run `wf_836460fa-e8a` plus the repair modes term-cards (twice), cited-cards, finalize and audit; run report `complete: true`, 0 gaps (2026-11-21). Before the post-digest sequence, the three identical Finding Card pairs under the introduction (1|j = 1|g, 1|k = 1|h, 1|l = 1|i, same body and statement fingerprint) were deleted `into` the card they repeat; the structure Card moved from 1|m to 1|j. Backup before that: `D:\zt\backups\zt-before-bedford2026learning-postdigest.sqlite3`.

## Stage 1: fold

Date 2026-11-21, live box (owner's request, overnight authority). Backup: `D:\zt\backups\zt-before-bedford2026learning-fold.sqlite3` (`VACUUM INTO`, integrity ok).

The digest left 25 Model Cards. The decision by nature, one row per Model Card:

| Model Card | Model (surviving head) | Reason |
|---|---|---|
| 3\|a | framework, head 3\|a | the conceptual framework of §II.B: binary latent match state, board hiring under frictions, two channels |
| 1\|c | framework, 3\|a | the introduction's verbal version of the framework (same latent evolving state, board and investors) |
| 4\|a | framework, 3\|a | §II.B.3 learning after appointment: the framework's third block (board and market on different information sets) |
| 10\|a | learning model, head 10\|a | §V Bayesian learning model: market belief over two types from performance and event signals |
| 1\|d | learning model, 10\|a | the introduction's verbal version of the learning model (early success, asymmetric learning) |
| 11\|a | learning model, 10\|a | §V.A the fixed-match-quality case of the same model |
| 12\|a | learning model, 10\|a | §V.B the time-varying case (first-order Markov states, two-step update) of the same model |
| 13\|b | learning model, 10\|a | §VI the two-state Markov process: the same state process as 12\|a and 18\|a |
| 16\|a | learning model, 10\|a | Appendix C.A: the formal environment, signals and two-step filter of the same model |
| 17\|a | learning model, 10\|a | Appendix C.B: the fixed case, formal |
| 18\|a | learning model, 10\|a | Appendix C.C–D: the time-varying case and simulations, formal |
| 19\|a | learning model, 10\|a | Appendix C.E: maps the model's uncertainty p(1−p) to return volatility |
| 20\|a | learning model, 10\|a | Appendix C.F: maps the model's transition risk to the within-CEO variance |
| 5\|a | own (empirical) | mixed-effects variance decomposition: its own estimand (economic importance of match effects) |
| 6\|a | own (formal) | the directional-distance-function measurement model: a construction, not a test |
| 7\|a | validation, head 7\|a | the score's validation against traditional match effects |
| 14\|a | validation, 7\|a | Appendix A: correlations among alternative implementations of the same score (robustness of the same measure on the same data) |
| 8\|a | own (empirical) | forced turnover on match-quality deciles: its own estimand |
| 9\|a | own (empirical) | annual stock returns on match-quality changes: its own estimand |
| 9\|b | own (empirical) | announcement abnormal returns, event data: its own estimand and data |
| 11\|b | volatility, head 11\|b | volatility on tenure deciles and initial match quality (Table 6) |
| 12\|b | volatility, 11\|b | Table 7: the same volatility–tenure regression split by within-tenure variance (heterogeneity of the same estimand on the same panel) |
| 13\|a | own (empirical) | frictions and initial match quality, first-year cross-section |
| 13\|c | own (empirical) | frictions and state transitions (Table 9 panel B): its own estimand and panel |
| 13\|d | own (empirical) | industry shocks and state transitions (Table 10): its own regressors and estimand |

The book holds **12 models**: 3 formal (3\|a framework, 6\|a measurement, 10\|a learning model) and 9 empirical (5\|a, 7\|a, 8\|a, 9\|a, 9\|b, 11\|b, 13\|a, 13\|c, 13\|d). One row needed judgement and was decided under the owner's overnight authority: the framework (3\|a) and the learning model (10\|a) stay two models. They share the binary latent state, but the framework asks how a board hires under frictions and guides every analysis of the paper, while the learning model asks how the market learns the type from signals, is presented as its own model in §V and Appendix C, and fills in only the market half of the framework's third block.

Method: the per-card fold script of the IMEN1 passes (`fold.mjs` driven by `drive.mjs`, session scratchpad `fold/`), one Card per run: strip **结构**/**引入**, mask every Link to a Model Card or part, park the parts and move the Direct note chains to the section anchors, retype (facet, emphasis core, level cleared), move the head `alone` into the surviving head's main line, return the parts, restore the body with the rename map. The verbal versions 1\|c and 1\|d went `after` the setting card (top of the main line); the others in book order at the end. The **引入** lines of 11\|a, 17\|a and 18\|a named parts of 10\|a, now the model's own parts, and were dropped. All 13 runs returned ok.

Facets given: 3\|a: 直觉章：引言的匹配质量框架 (1\|c), 第 2 章：任命后的学习 (4\|a). 10\|a: 直觉章：引言的简约学习模型 (1\|d), 第 1 章：固定匹配质量下的学习 (11\|a), 第 2 章：随时间变化的匹配质量 (12\|a), 第 3 章：两状态马尔可夫过程 (13\|b), 附录章：环境、信号与两步滤波 (16\|a), 附录章：固定匹配质量下的学习 (17\|a), 附录章：时变匹配质量与模拟 (18\|a), 附录章：收益波动率映射 (19\|a), 附录章：转换风险的方差映射 (20\|a). 7\|a: 附录章：替代指标之间的相关 (14\|a). 11\|b: 第 4 章：按任职期内方差分样本 (12\|b).

Gate:

- `model-survey.mjs`: exactly the 12 heads above (3\|a 15 parts, 10\|a 71, 7\|a 75, 11\|b 58).
- `check-model.mjs`: every former head is a part. The folded chapters are `unanchored` (3\|a\|a\|1, \|6; 10\|a\|a\|12–14 among others; 7\|a\|a\|7; 11\|b\|a\|7), left to the settings and restructure passes as in the earlier books.
- `list_broken_links` (agent): `{"broken":[]}`. Term relink: 0 Cards changed.

## Stage 2: settings (strict)

Date 2026-11-21, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-bedford2026learning-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The moves, the deletes and the creation of the three notation cards ran as six `apply_batch` tree batches (3\|a, 6\|a, 10\|a, 7\|a, 11\|b, each previewed and committed under its plan hash, `links_that_will_break` empty in all) and two reorder batches (the Side order of the folded appendix chapters of 7\|a and 11\|b). A card that was merged was first moved out of its Direct chain as a leaf (parked as a Side of the Model Card), so that its derivation chain rose into the chapter, and was then deleted `into` the card it restates. Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the equation numbers (`\tag{…}`, `\quad (N)`, `(Cn)`) of the old and new bodies, and accepted a new display formula only when it was carried byte for byte from a merged card; every written card was read back and compared with the intended text (0 differences). No display formula and no equation number changed in a card that stays, and every display formula of a deleted card stands in the card it was merged into. No `card_locked`, `scope_refused` or `read_view_changed` error occurred. The Literature tree went from 882 Cards to 874: 11 Cards were deleted `into` the cards they restate, and 3 notation cards were created (3\|a, 6\|a, 10\|a).

### 3\|a (formal)

```
3|a|a   设定：二元潜在匹配状态、董事会与先验信念                原始设定
├ a  符号表与读法 (new; blocks: 匹配状态、摩擦与先验, 决策与命题, 术语)
├ b  设定：概述（引言） (from 3|a|a|1|a, the verbal version of 1|c)  原始设定
├ c  假设：董事会只最大化预期匹配质量，摩擦只经两条渠道进入 (from 3|a|a|2, main line)  ← 3|a|a
└ d  设定：任命后的信息集 (from 3|a|a|6|a, the setting of former 4|a)  ← 3|a|a
```

- The assumption left the main line, so the decision (1), the no-information derivation and the proposition renumbered to 3\|a\|a\|2–4, and the chapter of former 4\|a is now 3\|a\|a\|5. The two folded chapters lost their setting cards; their mechanism and conclusion chains rose into their Side slot `a` (3\|a\|a\|1\|a, \|1\|a\|1; 3\|a\|a\|5\|a, \|5\|a\|1). The example of the introduction (former 3\|a\|a\|1\|a\|a) was first moved to its chapter as 3\|a\|a\|1\|b, since a move takes a card's Side successors with it.
- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**. The symbol and term tables of the setting card went into the notation card (11 rows, 2 terms). Definitions added at first use in the setting card: $\mathcal{C}(\mathcal{F})$ as the feasible set of candidates, $c$ as one candidate, and $\mathcal{I}_0(c,\mathcal{F})$ as the pre-appointment information (all three from the deleted symbol table).
- `d` dropped its restatement of the binary state and its two values (`repeat` against the setting card) and names them in **起点**.

### 6\|a (formal)

```
6|a|a   设定：决策单元、投入、产出与同年份比较集    原始设定
└ a  符号表与读法 (new; blocks: 单元、投入与产出, 方程 (2) 的分解, 规划、方向与效率, 术语)
```

- Only the setting card was a setting. It is now in the format, with six items; its tables went into the notation card (19 rows, 2 terms), which also covers the indices $i$ and $r$. Definitions added at first use: the focal unit $o$, the inputs $x_{ij}$ and outputs $y_{rj}$ (from the deleted symbol table), and the four components of (2) by name.
- The direction card (`方向`, 6\|a\|a\|2) and the definition of relative efficiency (7) (6\|a\|a\|4) stay on the main line: neither is a setting by facet or title, and both derive from the programme (3)–(6).

### 10\|a (formal)

```
10|a|a  设定：市场在两型匹配质量之间持有信念，每期观察两个信号 (§V; App. C.A merged)   原始设定
├ a  符号表与读法 (new; blocks: 类型、时点与先验, 两个信号, 信念, 转移, C.E, C.F, 推导中的辅助符号, 术语)
├ b  设定：概述（引言） (from 10|a|a|1|a, the verbal version of 1|d)                       原始设定
├ c  假设：好匹配下绩效更高、事件更可能，且两信号条件独立 (from 10|a|a|2; App. C.A assumption merged)  ← 10|a|a
├ d  设定：变体一：匹配质量固定 (from the §V.A setting 7|a; App. C.B 11|a merged)            ← 10|a|a, c
├ e  设定：变体二：时变匹配质量 (from the §V.B setting 8|a; App. C.C 12|a and §VI 9|a merged) ← 10|a|a, c
├ f  设定：收益波动率映射 (from the App. C.E setting 13|a)                                    ← 10|a|a
│    f|a 假设：风险中性定价，一次性资产，延续价值为零 (from 13|a|4)                           ← f
└ g  设定：转换风险的方差映射 (from the App. C.F setting 14|a)                                ← e
     g|a 假设：测量误差独立同分布且与状态路径独立 (from 14|a|1)                               ← g
```

(Addresses in parentheses are the former 10\|a\|a\|… addresses.)

- Merged (deleted `into` the card each restates, Links re-pointed): the App. C.A environment (former 10\|a\|a\|10\|a) into the setting card; the App. C.A assumption (10\|a\|a\|10\|a\|1) into `c`; the App. C.B fixed-type setting (11\|a) into `d`; the App. C.C time-varying setting (12\|a) and the §VI two-state Markov setting (9\|a) into `e`.
- Display formulas carried byte for byte: into the setting card, all five of App. C.A ((C2) in its bracket-matrix form, (C3), (C4), (C5)(C6), (C7)(C8)), each introduced as the appendix's writing beside the §V form; into `c`, the assumption's (C4) and (C5)(C6); into `e`, the §VI (C2) with `\tag{C2}` and the C.C forecast $\tilde p_{t+1}=P_{G\to G}p_t+P_{B\to G}(1-p_t)$. In consequence (C2) now stands in three cards (setting card, `e`, and `g`, which keeps its own copy) and (C4)–(C6) in two (setting card and `c`). They were kept because no display formula may be lost; they differ only in matrix brackets and in whether the side conditions sit inside the display.
- The main line lost the assumption and every chapter setting, so the chapters renumbered: 第 1 章 is now 10\|a\|a\|6, 第 2 章 \|7, 第 3 章 \|8, the five Appendix C chapters \|9–\|13; the derivations of each chapter rose into its Side slot `a`.
- Premises reversed: `e` rested on the proposition (9) (a chapter card, `order` fault) and the App. C.C setting rested on the proposition (C10); both are pointers in **说明** of `e` now. The variants `d` and `e` name each other without a relation word, so no `relaxes` relation is made (one first wording in the setting card made a `relaxes` relation to `e`; it was rewritten).
- Statements moved: the conditional independence of the two signals left the setting card (it is `c`'s third statement); the setting card took the App. C.A remark that the market knows the parameters; `e` took $P_{G\to B}=1-P_{G\to G}$ from the mechanism card 10\|a\|a\|7\|a, whose `**记号**` section (`stated`) became `**符号所在**`, two pointers to the setting card and `e`.
- Definitions added at first use: $N(0,\sigma_\varepsilon^2)$, $f(y_t\mid\theta)$ and $\sigma_y^2$ in the setting card; $q_{G,0}$, $q_{B,0}$, $\lambda$ and $e$ beside (C5)(C6); $\mathcal I_t$ in `f`; $P_t$ in `f|a`; $\bar Z$ in `g`.
- Every setting card is in the format; the five symbol tables and the term tables went into the notation card (66 rows, 12 terms). One row per letter names every use: $\sigma_\varepsilon^2$ (performance noise; fundamental noise in C.E; measurement error in C.F), $\mu_G$, $\mu_B$, $p$ (generic belief; the C.E posterior (C18)), and $C$ (the appendix's equation prefix, which the check reads as a letter inside `\quad(C2)`). The auxiliary rows ($L_\theta$, $L_t$, $L_0$, $J_t$, $D_t$, $\tau$, $J$, $D$, $m$, $M$, $\bar Z_i$, $\mathrm{Var}_T(Z_t)$, $\mathrm{Var}^M_{iT}$) name the chapter cards where a reader meets them first; the two `notation` faults ($L_\theta$, $L_t$) are closed by them.

### 7\|a (empirical)

The folded Appendix A chapter (former 7\|a\|a\|7, now 7\|a\|a\|8) carried its own data card, 变量表, measure, design with two assumptions, and a specification ladder (a 规格：基准 with ten variants). It was dissolved in one batch of 8 items:

- Deleted `into` the card each restates: the Appendix A data card into the data card 7\|a\|a (item 7); its 变量表 into 7\|a\|a\|a (five rows added: $\beta^{*}_{mjt}$, $\delta_{j}$, $q_{mjt}$, $\phi_{gt}$, $\rho$; the $\theta$ row names both readings); its design into the design 7\|a\|a\|2 (variation source, comparison, two threats, two assumption lines).
- Moved: the two assumptions to the design as 假设 6 and 假设 7 (7\|a\|a\|2\|f, \|g; titles and facets renumbered); the measure of the two-stage score under the data card (7\|a\|a\|c); the ladder of Appendix A to the main line after the variance decomposition as 7\|a\|a\|5, facet `规格：替代实现的相关` (no second `规格：基准`), its ten variants under it unchanged; the two Appendix A chapters into the appendix chapter's Side slot `a` (7\|a\|a\|8\|a, \|8\|a\|1), ahead of its limits card (\|8\|b).
- The two measures 7\|a\|a\|b and \|c now have a one-line summary, **起点** and **内容**.

### 11\|b (empirical)

The folded chapter of former 12\|b (Table 7, now 11\|b\|a\|7) carried a data card, 变量表, two measures, a hypothesis, a design with four assumptions and a ladder (a 规格：基准 with two variants). It was dissolved in one batch of 12 items:

- Deleted `into` the card each restates: the data card into 11\|b\|a (item 6: the Table 7 sample, 15,589 and 15,586 observations); the 变量表 into 11\|b\|a\|a (rows $Z_{i,t}$, $\alpha$, $\theta$, $\tau_{t}$ added; the rows $\delta$, $\gamma_{d}$, $\lambda_{t}$ and $Tenure^{d}_{i,t}$ name the Table 7 readings, since Table 7 uses $\delta_d$, $\gamma$, $\lambda$ for other coefficients than Table 6); the design into 11\|b\|a\|2.
- Moved: the four assumptions to the design as 假设 4–7 (11\|b\|a\|2\|d–g, renumbered); the stable-subsample specification under the baseline as 11\|b\|a\|3\|d, facet `规格：稳定子样本`, with a `依赖` line to the baseline and **相对基准的改动** (sample, the added regressor $Z_{i,t}$ and its interaction, firm and year effects), its two variants under it; the two measures under the data card (11\|b\|a\|d, \|e); the hypothesis into the chapter (11\|b\|a\|7\|a) with the chapter's two sub-chapters as its Direct chain (\|7\|a\|1, \|7\|a\|2), ahead of the limits card (\|7\|b). The two tests of the chapter already cite their assumptions by Link (now 假设 6 and 假设 4).

### Other empirical models

5\|a, 8\|a, 9\|a, 9\|b, 13\|a, 13\|c, 13\|d had only `start` faults: every sample, measure and description card of their data trees (13 cards in all, including 11\|b\|a\|b, \|c) now has a one-line summary, **起点** with its `依赖` lines and **内容**; a `见 [[…]]` line that repeats a `依赖` target was dropped, and 8\|a\|a\|b's `见 [[8|a|a]]：…` became its `依赖` line. No specification, design or assumption changed.

The **结构** sections of 3\|a, 6\|a, 10\|a, 7\|a and 11\|b were rebuilt from the build order.

### Gate

- `check-settings.mjs` on the three formal heads (3\|a, 6\|a, 10\|a): `settings: all in the setting part, as a dependency tree`, no fault; every `reads:` symbol is defined in the card that reads it first.
- `check-settings.mjs` on the nine empirical heads (5\|a, 7\|a, 8\|a, 9\|a, 9\|b, 11\|b, 13\|a, 13\|c, 13\|d): `settings: the data tree and the specification ladder are in place`, no fault.
- `check-model.mjs 3|a`: 16 members, loose 1, `unanchored` on the two folded chapters (3\|a\|a\|1, \|5). `6|a`: 9 members, loose 0, `self_sufficient` true. `10|a`: 67 members, `unanchored` on the introduction chapter and chapters 6–13; two `relaxes` relations (10\|a\|a\|5 and \|6 to chapter \|7) come from chapter wording that this pass did not touch. `7|a`: 72 members, `unanchored` on 7\|a\|a\|8. `11|b`: 55 members, `unanchored` on 11\|b\|a\|7. Left to the restructure, as after stage 1.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"links_repaired":0,"book":"bedford2026learning","links_added":96,"cards_changed":37,"ok":true,"rounds":1,"applied":true,"cards_refused":0}`. All twelve settings checks were rerun after it and stay clean.

Cards per model subtree, before → after: 3\|a 23 → 24, 6\|a 21 → 22, 10\|a 71 → 67, 7\|a 85 → 82, 11\|b 58 → 55; 5\|a, 8\|a, 9\|a, 9\|b, 13\|a, 13\|c, 13\|d unchanged (28, 26, 28, 25, 32, 25, 23).

### Open

1. All folded chapters are unanchored (3\|a\|a\|1, \|5; 10\|a\|a\|1, \|6–\|13; 7\|a\|a\|8; 11\|b\|a\|7). The restructure builds the spine.
2. 10\|a: the Appendix C chapters (\|9–\|11) restate the §V derivations in appendix notation (the two-step filter (C8)–(C10) against (9) in \|2–\|3; the fixed case (C11)–(C15) of \|10 against chapter 1 \|6; the time-varying bound of \|11 against chapter 2 \|7). This pass merged only the setting cards; the restructure decides whether the appendix derivations merge into the §V chapters.
3. 10\|a: (C2) stands in three setting cards and (C4)–(C6) in two (see above). The owner may decide to keep one copy each.
4. 10\|a: the definition of transition risk (C31) (now 10\|a\|a\|13\|a\|1) stays in its chapter, since it rests on the derivation (C30); the restructure may treat it as a result card. In 6\|a the direction choice (6\|a\|a\|2) and the definition (7) stay on the main line for the same reason.
5. 10\|a and 3\|a keep an introduction chapter (`直觉章`) beside the overview setting `b`; the restructure decides whether these verbal chapters become the intuition spine or merge.
6. 7\|a: the Appendix A ladder 7\|a\|a\|5 stands on the main line as a second, non-baseline specification; the restructure places its estimates (the appendix chapter 7\|a\|a\|8) on the evidence spine.
7. 11\|b: the folded chapter now opens on its own hypothesis card (11\|b\|a\|7\|a), a second hypothesis beside 11\|b\|a\|1; the restructure decides whether the head's hypothesis absorbs it.
