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

## Stage 3: restructure

Date 2026-11-21, live box (owner asleep, overnight authority: every decision below was taken without asking and is recorded here), under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; chapter cards and leads written by hand). Backup: `D:\zt\backups\zt-before-bedford2026learning-restructure.sqlite3` (`VACUUM INTO` through `node:sqlite` read-only, `PRAGMA integrity_check` ok). Moves, creates and deletes ran as seven `apply_batch` tree batches (3\|a: 4 items, one `insert_card`; 6\|a: 3 + 3 items, two `insert_card`; 10\|a: 17 items; 11\|b: 13 items; 7\|a: 27 items), each previewed and committed under its plan hash; `links_that_will_break` was empty in every preview and every `deleted` set was the intended one. Text ran as `edit_card` calls under `-Allow`; before each text write a script compared the display formulas and the equation numbers (`\tag{…}`, `\quad (N)`, `(Cn)`) of the old and new bodies and accepted a new display formula only when it was carried byte for byte from a deleted card; every written card was read back and compared with the intended text (0 differences). After the pass a script compared the whole book with the backup (Term macros stripped): 1,194 display formulas (1,087 distinct) and 160 equation tags (43 distinct) before and after, nothing lost and nothing added. No `card_locked`, `scope_refused` or `read_view_changed` error occurred. The Literature tree went from 873 to 867 Cards: 3 chapter cards created (3\|a: 1, 6\|a: 2) and 9 cards deleted `into` the card that holds their content (10\|a: 4, 11\|b: 3, 7\|a: 2).

One tool fault was met and worked around: in an `apply_batch` that holds two `insert_card` items, a Link in the second insert's body to a card the first insert had renumbered was refused (`link target … does not exist`, the body Link was translated one rename too far), and a body Link `[[@@t0]]` is refused (`invalid link macro target`). The 6\|a batch was split in two and the second chapter's premise line was written by `edit_card` afterwards.

### 3\|a (formal): the intuition spine

```
3|a|a    设定 (setting part as stage 2 left it: a 符号表, b 概述（引言）, c 假设, d 任命后的信息集)
3|a|a|1  第 1 章：直觉 — 匹配质量是配对的潜在状态，随环境移动，并在任期内被逐步学习      ← b, 设定
    a 机制 → a|1 结论（三个可检验方向）;  b 例子
3|a|a|2  第 2 章：聘用决策 (new) — 董事会按任命前信号选人，摩擦经两条渠道压低初始匹配质量  ← 设定, c
    a 决策 (1);  b 结果：摩擦的两条渠道 (former 命题) → b|a 推导：无信息极限 (proof)
3|a|a|3  第 3 章：任命后的学习 — 董事会与市场在不同信息集上学习，转换风险使信念不能永久收敛  ← 第 2 章, d
    a 机制 → a|1 结论
```

- Open item 4 (introduction chapter beside overview `b`): the introduction chapter became 第 1 章：直觉, the lead of the spine; `b` stays the verbal setting it rests on (one is a setting, the other a chapter, so no merge). A rests-on Link to `b` alone left the chapter unanchored (`b` is itself an original setting with no path to the setting card), so the chapter also rests on the setting card.
- The flat §II.B.2 chain (decision (1), the no-information derivation, the proposition) left the main line into a new chapter card; the proposition became the result `结果：摩擦的两条渠道`, its derivation the proof branch. The chapter of former 4\|a is now 第 3 章 and rests on 第 2 章 (its conclusion uses the selection rule) and on `d`.
- Open item 1: both folded chapters are anchored. Chapter cards have **直觉** (lead of 20+ characters), **本章结果** and **承上启下** with `依赖` lines; the old 问题/概要/关联 text stays below. `d`'s **直觉** said `第 2 章`; it now says `第 3 章`.

Network: 设定 → {1, 2, 3}. Three chapters, so no **分部**.

### 6\|a (formal): the intuition spine

```
6|a|a     设定 (a 符号表)
6|a|a|1   第 1 章：相对效率 (new) — 方向距离函数量出每个单元离当年前沿多远，并换成可跨规模比较的相对效率   ← 设定
    a 结果：相对效率 θ=1-β* (7) (former main-line |4)
        a 规划 (3)–(6) → a|1 方向 (a|1|a 对照：DDF 而非 DEA) → a|2 推导：规模可比        (proof chain)
6|a|a|2   第 2 章：两阶段剥离 (new) — 按年估计前沿去掉年份成分，再去掉企业成分，剩下的效率作为匹配质量的代理   ← 第 1 章
    a 第一阶段;  b 结果：匹配质量的代理 (former 第二阶段)
```

- Open item 3 (cards that derive from results): the definition (7) is placed as the result of chapter 1, with the programme, the direction choice and the scale derivation as its proof chain; the direction choice is a step of that chain (it uses the constraints (4) and (5) and feeds the scale derivation), not a setting. The second stage became `结果：匹配质量的代理`.
- Settings check after the move: $\beta$ is now met first in the result card; the result card got one sentence that defines $\beta$ and $\beta^{*}_{mjt}$, and the notation row of $\beta$ points to it ($\beta_o^{*}$ stays with the programme).

Network: 设定 → 1 → 2. Two chapters, no **分部**.

### 10\|a (formal): the intuition spine

```
10|a|a        设定 (a 符号表, b 概述（引言）, c 假设, d 变体一, e 变体二, f 收益波动率映射 → f|a, g 方差映射 → g|a)
10|a|a|1  第 1 章：直觉 — 早期成功更常出现于好匹配，市场的学习因此不对称           ← b, 设定
    a 机制 → a|1 结论
10|a|a|2  第 2 章：信念更新 (former App. C.A chapter card |9) — 对数似然比更新，不确定性 p(1-p) 随任期下降 (§V, App. C.A)   ← 设定, c
    a 结果：对数几率更新 (9) (former |3; App. C.A (C10) merged)
        a 推导：对数几率更新 (former |2);  b 推导：预测步 (C8) → b|1 推导：更新步 (C9)   (two proof branches)
    b 结果：不确定性随任期下降 (former |5) → b|a 推导：不确定性度量 (former |4)
    c 推导：事件信息含量 → c|1 结果：事件的信息含量 (former App. C.A chain)
10|a|a|3  第 3 章：固定匹配质量 (§V.A; App. C.B merged)                         ← 第 2 章, d
    a 对数几率累积 → a|1 结果 → a|2 → a|3 → a|4 结果：学习不对称
        a|4|a App. C.B proof: (C11) → 命题 → (C13) → (C14) → (C15) → 命题 (C15) → J 与 D
10|a|a|4  第 4 章：时变匹配质量 (§V.B; App. C.C–D merged)                        ← 第 2 章, e
    a 机制 → a|1 结果：转换风险保留不确定性 (a|1|a 例子：模拟;  a|1|b App. C.C proof: 范围 → 正下界 → 命题 → 机制 → 命题)
10|a|a|5  第 5 章：两状态马尔可夫过程 (§VI.B)                                    ← 第 4 章, e
10|a|a|6  第 6 章：收益波动率映射 (App. C.E)                                      ← 第 2 章, f
10|a|a|7  第 7 章：转换风险的方差映射 (App. C.F)                                  ← 第 4 章, g
```

**分部** (new in the Model Card): 设定与直觉 from `10|a|a`; 信念更新与不确定性（§V 与附录 C.A–D） from 第 2 章; 状态过程与可观测量（§VI.B、附录 C.E–F） from 第 5 章.

- Open item 2 (Appendix C chapters restating §V): decided by the source's own division of labour. §V states the results in words and Appendix C proves them, so each appendix derivation chain became the proof branch of the §V result it proves, and only cards that restate a §V card were deleted `into` it:
  - App. C.A: the proposition (C10) (identical to (9)) was deleted `into` the result (9), its display formula with (C10) carried byte for byte under **附录 C.A 的写法**; the two-step filter (C8)→(C9) became the second proof branch of (9); the event-informativeness chain stayed as result `c` of 第 2 章; the App. C.A chapter card itself became 第 2 章 (its old problem/summary text kept below the new sections).
  - App. C.B: the closing proposition (uncertainty resolves fast under G, slowly under B) was deleted `into` the §V.A asymmetry result `3|a|4`; the rest of the chain (C11)–(C15) and $J$, $D$ moved under it as its proof branch; the chapter card was deleted `into` 第 3 章, its (C11) display carried byte for byte under **附录 C.B 的概要**. The proposition (C15) `3|a|4|a|5` and the §V.A derivation `3|a|3` both state $\mathbb E[\tau\mid G]<\mathbb E[\tau\mid B]$; the (C15) card carries the `\tag{C15}` display and the doubt on divergence, so both stay.
  - App. C.C–D: the chain (range, positive bound, proposition, mechanism, proposition) moved under the §V.B result `4|a|1` as its proof branch; the chapter card was deleted `into` 第 4 章 (text under **附录 C.C 的概要**).
  - App. C.E and C.F derive results that §V does not state (the return-variance and within-spell-variance mappings), so they stay chapters (第 6、7 章).
- Open item 3: the definition of transition risk (C31) (`7|a|1`) stays a step of 第 7 章's derivation chain: it rests on (C30) and is used by (C34)–(C36), and the chapter card names it beside the chapter's result.
- Open item 7 (`relaxes`): both relations ran the wrong way (an earlier card "relaxing" a later chapter). The uncertainty result now says `允许匹配质量随时间变化的情形见第 4 章 [[…]]`, 第 3 章's 关联 line and the head's 关联 line say `参照`; `check-model` lists no `relaxes` relation now.
- Facets: 命题 → `结果：…` on the five §V results named above; chapter facets `第 k 章：…`. Stale chapter numbers fixed in `d` (`第 1 章` → `第 3 章`, twice) and `e` (`第 2 章` → `第 4 章`); 第 3 章's self-Link (left by the delete into it) now points to the App. C.B proof branch.

### Empirical models

- **7\|a (open item 5).** The appendix chapter became 第 3 章：替代实现（附录 A） under the Appendix A specification `7|a|a|5`, which stays on the main line after the baseline and the variance decomposition. Its two inner chapters (换第二阶段 / 换第一阶段) were dissolved as in riboni2026presidential: tables A1-A and A1-B and the 20 correlation estimates moved up as Side branches of the chapter (a–k panel A, l–v panel B, w limits), and the two inner chapter cards were deleted `into` the chapter, whose **本章估计** carries their lines. Chapter 1's lead was lengthened. Network: 规格 (8) → {方差分解, 第 1 章, 第 2 章}; 规格：替代实现 → 第 3 章.
- **11\|b (open items 1 and 6).** The second hypothesis card `7|a` (extension model, §V.B) was deleted `into` the head's hypothesis `11|b|a|1`, which now states it as **假说二（扩展模型，§V.B）** and has a widened title: the evidence spine has one hypothesis card, and both predictions are tested on the same panel. The two inner chapters of 第 4 章 were dissolved (table 7, the stable-subsample estimates b–d, the high-variance estimates e–g with their tests, limits h), and 第 4 章 rests on the stable-subsample specification `3|d` and on 第 1 章. Network: 规格 → {1 → 3, 4}; 设计 → 2.
- **9\|a (table panel).** The script-read Table 4 rows were Panel A: every estimate card's **估计** section carried the Panel A (forced turnover) value, standard error and N = 42,678, while the title, the 量级 line and the source text (s09.md: 结果见表 4 面板 B) give the Panel B (stock returns) coefficient. The table reference was corrected to `表 4 面板 B (k)（$\Delta$Match Quality$_{d=n}$ 行）` and value, SE and N were taken from Panel B (s21.md Table 4 Panel B): 0.450 (0.015), 0.208 (0.010), 0.084 (0.009), 0.449 (0.014), 0.451 (0.014), 0.419 (0.014), N = 41,353; the three tests' SE lines and the two rows of the head's 主要估计 table likewise. No other number changed. 8\|a cites Panel A, which is its table (checked against s08.md and the table: all six estimate cards match).
- **All nine.** A **直觉** lead was prepended to every data, hypothesis, design and specification card on the main line (37 cards); the short chapter leads of 7\|a 第 1 章 and 9\|b 第 3 章 were lengthened. Loose 2/2/1/2/2/2/2 → 0: each limits card off the spine got one plain reference line (`见设计 […]`, or `本卡属于第 k 章 […]`).

**结构** rebuilt (main line and one level under it) for 3\|a, 6\|a, 10\|a, 7\|a and 11\|b.

### Gate

| Model | members (main/side/shared/off) | loose | self_sufficient | network | premises |
|---|---|---|---|---|---|
| 3\|a | 17 (4/12/1/0) | 1 | true | clean | none |
| 6\|a | 11 (3/8/0/0) | 0 | true | clean | none |
| 10\|a | 63 (8/54/1/0) | 0 | true | clean (9 blocks, 2 layers) | 2 pointers |
| 5\|a | 22 (4/16/0/2) | 0 | true | clean | 2 pointers |
| 7\|a | 70 (9/60/0/1) | 0 | true | clean (10 blocks, 5 layers) | 1 pointer |
| 8\|a | 25 (6/18/0/1) | 0 | true | clean | none |
| 9\|a | 27 (6/20/0/1) | 0 | true | clean | none |
| 9\|b | 24 (7/16/0/1) | 0 | true | clean | none |
| 11\|b | 52 (8/42/1/1) | 0 | true | clean (10 blocks, 4 layers) | none |
| 13\|a | 31 (6/23/0/2) | 0 | true | clean | none |
| 13\|c | 25 (6/17/1/1) | 1 | true | clean | none |
| 13\|d | 24 (6/15/2/1) | 0 | true | clean | none |

- No closure, forward, reach_in or unanchored fault in any model. The two remaining loose Links (3\|a, 13\|c) are the legacy share of 10\|a's 第 5 章, left to stage 4.
- Premise hits, all pointers: 10\|a `b` (`形式化见 … (9) 式`) and `e` (`更新式 (9) 见 […]`); 5\|a `|1` and `|1|c` and 7\|a `|3|c` name table columns `(2)`, `(4)`, which the check reads as equation numbers.
- `check-settings.mjs`: 3\|a, 6\|a, 10\|a `settings: all in the setting part, as a dependency tree` (10\|a: advice lines only); the nine empirical models `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` (agent): `{"broken":[]}`. Whole-book formula comparison with the backup: no difference (see above).
- Term relink: `{"cards_refused":0,"ok":true,"links_repaired":0,"applied":true,"cards_changed":56,"links_added":174,"book":"bedford2026learning","rounds":1}`. All checks above were rerun after it and stay as listed; the **分部** lines of 10\|a are unchanged.

Cards per model subtree, before → after: 3\|a 24 → 25, 6\|a 22 → 24, 10\|a 67 → 63, 7\|a 82 → 80, 11\|b 55 → 52; 5\|a, 8\|a, 9\|a, 9\|b, 13\|a, 13\|c, 13\|d unchanged (28, 26, 28, 25, 32, 25, 23).

### Open (for stage 4, imports, and later passes)

1. Legacy shares to turn into **引入** lines or plain references: 3\|a ↔ 10\|a (3\|a's head 关联 and its loose Link to 10\|a's 第 5 章 `10|a|a|5`; 10\|a's head 对应 line to 3\|a's 第 3 章 `3|a|a|3`); 11\|b → 10\|a 第 3 章 `10|a|a|3` (the head hypothesis rests on its two predictions; 假说二 on 第 4 章 `10|a|a|4`); 13\|c → `10|a|a|5`; 13\|d → `10|a|a|5` and `10|a|a|4`.
2. Further import candidates the survey should weigh: 7\|a's measure `7|a|a|c` (两阶段匹配质量得分) restates 6\|a's construction (第 1–2 章), and 8\|a, 9\|a, 9\|b, 11\|b, 13\|a, 13\|c, 13\|d use the same score; 6\|a's setting item 6 takes the decomposition (2) from 5\|a; 13\|a's hypothesis is 3\|a's 结果：摩擦的两条渠道 (`3|a|a|2|b`); the 8\|a/9\|a/9\|b hypotheses are directions of 3\|a's 第 1 章 conclusion (`3|a|a|1|a|1`); 11\|b's 第 4 章 tests 10\|a 第 4 章 and uses 第 7 章's proxy (`10|a|a|7`) and 第 6 章's volatility mapping.
3. 10\|a: (C2) still stands in three setting cards and (C4)–(C6) in two (stage 2 open item 3).
4. 10\|a: chapters 3 and 4 hang under the setting block in the network (their derivations rest mostly on `d`, `e` and `c`), although their 承上启下 rests on 第 2 章; the network is clean. Two restatements stay inside proof chains: (C15) `3|a|4|a|5` beside `3|a|3`, and the App. C.C proposition `4|a|1|b|4` beside the bullet of `4|a|1`.
5. Step 3 card text was not redrafted: result and derivation cards keep their digest bodies (no `**为什么**` section on most results), the old chapter cards keep their 问题/概要/关联 text below the new sections, and the merged appendix chapter texts stand under `**附录 C.x 的概要**` headings.
6. Tree unlocked throughout; no `card_locked` met.

## Stage 4: imports and explanations

Date 2026-11-21, live box (owner asleep, overnight authority: every decision below was taken without asking and is recorded here), under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent; no DeepSeek job, see step 8, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-bedford2026learning-imports.sqlite3` (`VACUUM INTO` through `node:sqlite` read-only, `PRAGMA integrity_check` ok). Hand text ran as `edit_card` calls under `-Allow` (bedford2026learning Cards only), one Card at a time, 38 writes on 34 Cards: before each write a script compared the display formulas and the equation numbers (`\tag{…}`, `\quad (N)`, `(Cn)`) of the old and new bodies and allowed a display to disappear only when it was named in the write's spec; every written Card was read back (`same` on all). Import lines: 22 written by `imports.mjs apply` as premises (`单卡`), 12 of them upgraded to `整块` by a second `apply`, then their text after `：` rewritten by hand to say what the model uses. One script fault on our side: the shell collapsed `\\` in a heredoc, so the first write of `10|a|a|c` had `$sigma_y^2$`, `gg`, `lambda`; `check-settings` caught it (uncovered letters) and the next write restored `\sigma`, `\gg`, `\lambda`. No `card_locked`, `scope_refused` or `read_view_changed` error occurred.

Before any edit: `check` exit 1 on 3\|a (legacy share: the Model Card's **关联** Link to 10\|a's 第 5 章 `10|a|a|5`), exit 0 on the other eleven; `plan` proposed only that line. False shares on 10\|a (`3|a|a|3`), 11\|b (`10|a|a|3`), 13\|c (`10|a|a|5`), 13\|d (`10|a|a|4`, `10|a|a|5`); every hypothesis named its theory with `参照`, so no premise reached another model. Order: 3\|a, 5\|a, 6\|a (lenders), then 10\|a, then the empirical tests.

### 3\|a (formal, framework)

- No import. The legacy share was not a premise (nothing in 3\|a rests on `10|a|a|5`): the **关联** line now reads `参照 [[10|a]]，状态演变的形式写法（两状态马尔可夫过程）见学习模型的第 5 章。` (a Link between Model Cards, no member). The stale line `对照 [[3|a|a|3]]，任命后学习不在本卡，本卡只到任命时点` (wrong since stage 3 moved the chapter in) now reads `参照 [[3|a|a|3]]，任命后的学习在第 3 章。` Legacy shares 1 → 0, members 17 → 16, loose 1 → 0.
- Copy advice judged, comparisons, not copies: `3|a|a|1` ~ `10|a|a|1` and ~ `10|a|a|4`; `3|a|a|3` ~ `10|a|a|4` and ~ `10|a|a|7`. They come from `对应` lines in the chapters' old **关联** text. 3\|a's chapters state the framework in words for both the board and the market; 10\|a's chapters derive the market half formally (stage 1: the two models stay two). Left as advice.
- Decision: 10\|a imports nothing from 3\|a either. Both setting cards write the binary latent state and the prior, each as a primitive of its own model; 10\|a uses none of 3\|a's board, frictions or selection rule.
- 3\|a now lends `3|a|a|1|a|1` (结论：三个可检验方向) to 8\|a, 9\|a, 9\|b, 13\|c, 13\|d and `3|a|a|2|b` (结果：摩擦的两条渠道) to 13\|a, all `单卡`.

### 5\|a (empirical, variance decomposition)

- No import; `check` exit 0, `plan` `nothing to import`. It lends the specification `5|a|a|2` (方程 (2)) to 6\|a.

### 6\|a (formal, DDF measurement)

- Decision: 6\|a takes the decomposition (2) from 5\|a. The setting card's item 6 uses its symbols ($\alpha_t$, $\delta_j$, $\eta_m$, $\xi_{mj}$) and 第 2 章 strips its year and firm terms. The setting card's **起点** gained `依赖 [[5|a|a|2]]：第 6 项的分解符号取自它的方程 (2)（经 **引入**）。` (item 6 keeps its `参照 [[5|a]]`). `plan`: `premise: 单卡 [[5|a|a|2]]`; `apply`: `wrote 1 import line(s); tolerated 0 -> 2`. Line: `单卡 [[5|a|a|2]]：方程 (2) 把资产收益率分成年份、企业、管理者与匹配四个成分；设定第 6 项沿用它的符号，第 2 章按它逐项剥离年份项与企业项`.
- **Decision: the line stays `单卡`; the `整块` upgrade that `plan` proposes was not applied.** It would bring 5\|a's data card and design (`5|a|a`, `5|a|a|1`) into a formal measurement model that uses only the equation's decomposition, by the same rule as a formal model that takes an estimate (`单卡`). Tolerated 2 (`5|a|a`, `5|a|a|1`); `plan` keeps proposing the upgrade (advice, not a check fault).
- Symbols: 6\|a's notation card already lists the four components; `check-settings` clean, no `沿用` line.
- 6\|a now lends `6|a|a|2|b` with its closure (the whole model) to the eight empirical models that use the score.

### 10\|a (formal, Bayesian learning model)

- No import. The false share `对应 [[3|a|a|3]]` in the Model Card's **关联** now reads `对应 [[3|a]]：框架的第 3 章写董事会一侧，用同一贝叶斯逻辑更新，但信息集不同。` False shares 1 → 0, members 63 → 62, tolerated 2 → 0.
- Copy advice judged: `10|a|a|1` ~ `3|a|a|1` (as above); `10|a|a|4` ~ `11|b|a|7` (a theory chapter and the empirical chapter that tests it, joined by the old `对应` text). Left as advice.
- **Stage 2 leftover (C2), (C4)–(C6), decided.** Inside one model the copy rule applies where a card rests on another and restates it with no change; a display was removed only when the same formula, with the same entries and conditions, stays in the card it rests on.
  - (C2): kept once, in the setting card `10|a|a` (item 8, the App. C.A writing, where the notation card says $P$, $P_{G\to G}$, $P_{B\to G}$ are first met). Variant two `10|a|a|e` (item 2, `pmatrix` with `\tag{C2}`) and the variance mapping `10|a|a|g` (item 2, `bmatrix` without the final period) restated it: both now say `转移矩阵是设定卡 [[10|a|a]] 第 8 项的 (C2)`; `e` keeps that both probabilities lie in the open interval $(0,1)$ and its reading of the two rows; `g` adds `（变体二 [[10|a|a|e]] 沿用它）`. Bracket style and tag form are the only differences lost.
  - (C4): kept once, in the setting card (item 4). The assumption card `10|a|a|c` (item 1) restated it identically; it now says `附录 C 还设两类匹配下绩效同方差 $\sigma_y^2$，即设定卡 [[10|a|a]] 第 4 项的 (C4)。`
  - (C5)(C6): **both copies kept.** The setting card's display carries the definitions $\Pr(X_t=1\mid G)\equiv q_G(t)$ and $\Pr(X_t=1\mid B)\equiv q_B(t)$; the assumption card's display carries $q_G(t)\gg q_B(t)$ and $\lambda>0$. Neither holds the other's content, so neither is a restated copy. The assumption card's **说明** now says which card writes which part.
  - `check-settings 10|a` after the writes: `settings: all in the setting part, as a dependency tree`.
- 10\|a now lends `10|a|a|e` (with closure `10|a|a`, `10|a|a|c`) to 13\|c and 13\|d; `10|a|a|3|a|1`, `10|a|a|3|a|4`, `10|a|a|4|a|1` (`单卡`) and `10|a|a|6|a|4`, `10|a|a|7|a|5` (`整块`) to 11\|b.

### Empirical models: the predictions they test

Each `假说：` part now rests on the theory's result (`依赖` in place of `参照`, with `（经 **引入**）`), and the result is imported `单卡`; `plan` proposed no upgrade for these, as the skill says for a result only 假说 parts rest on.

| Model | Hypothesis | Rests on (单卡) | Replaced |
|---|---|---|---|
| 8\|a | `8|a|a|1` | `3|a|a|1|a|1` first direction (turnover) | `参照 [[3|a]]` |
| 9\|a | `9|a|a|1` | `3|a|a|1|a|1` first direction (pricing) | added; `参照 [[9|c]]` kept |
| 9\|b | `9|b|a|1` | `3|a|a|1|a|1` first direction (pricing) | `参照 [[3|a|a|3]]`; `参照 [[10|a]]` kept |
| 13\|a | `13|a|a|1` | `3|a|a|2|b` two channels | `参照 [[3|a]]` |
| 13\|c | `13|c|a|1` | `3|a|a|1|a|1` third direction (frictions → initial match; transitions → environment) | `参照 [[3|a]]` |
| 13\|d | `13|d|a|1` | `3|a|a|1|a|1` third direction (transitions → environment) | `参照 [[10|a|a|4]]` |
| 11\|b | `11|b|a|1` 假说一 | `10|a|a|3|a|1` (convergence, volatility falls), `10|a|a|3|a|4` (asymmetric learning) | `参照 [[10|a|a|3]]` |
| 11\|b | `11|b|a|1` 假说二 | `10|a|a|4|a|1` (transition risk keeps uncertainty) | `参照 [[10|a|a|4]]` |
| 7\|a | `7|a|a|1` | `6|a|a|2|b` (the residual is the match-quality proxy), inside the `整块` import below | `参照 [[6|a]]` |

Decisions: the 8\|a, 9\|a, 9\|b hypotheses test the framework's first direction (match quality enters the board's turnover decision and the investors' pricing), not 3\|a's 第 3 章, which only says beliefs keep updating; 13\|c's null and 13\|d's conjecture test the third direction (`3|a|a|1|a|1` states both halves: initial match ~ frictions, transitions ~ environment). 13\|d's conjecture is the authors' own (`这是作者的猜想`), so it rests on the framework direction and on the transition process (below), not on a 10\|a result.

### Empirical models: the shared measure and the theory's mappings

- **The match-quality score (all eight score users).** The two-stage DDF score of 6\|a is a measure shared by 7\|a, 8\|a, 9\|a, 9\|b, 11\|b, 13\|a, 13\|c, 13\|d. Each model's first measure card that uses it gained `依赖 [[6|a|a|2|b]]：匹配质量得分是 DDF 度量模型第 2 章的残差化效率（经 **引入**）。`: `8|a|a|b`, `9|a|a|b`, `9|b|a|c`, `11|b|a|c`, `13|a|a|b`, `13|c|a|b`, `13|d|a|b`. `plan` round 1: `单卡 [[6|a|a|2|b]]`; round 2: `upgrade: 整块 [[6|a|a|2|b]]` (closure: the setting card, the notation card, 第 1 章 with its result and proof chain, 第 2 章, the first stage; +6 members besides the root). Round 3: `nothing to import`.
- **7\|a (copy turned into a variant).** `7|a|a|c` (测量：方向距离函数的相对效率，再对固定效应残差化) restated 6\|a's construction (第 1–2 章). It now rests on `6|a|a|2|b` (**起点** `依赖 … 两阶段构造取自 DDF 度量模型的第 1、2 章（经 **引入**）`) and opens its **内容** with `本卡沿用引入的构造，只记附录 A 主做法的口径：前沿按同年度全部企业估计，第二阶段对企业固定效应残差化。`. The rest of the card (directions, $\theta_{mjt}=1-\beta^{*}_{mjt}$, $q_{mjt}$, the **直觉**) stays: it is the Appendix A main implementation that the robustness ladder `7|a|a|5` varies, and its symbols are 7\|a's 变量表 rows. Not deleted.
- **11\|b, the theory's mappings.** 假设 1 `11|b|a|2|a` (volatility moves with uncertainty about match quality) now rests on `10|a|a|6|a|4` (收益方差与 $p(1-p)$ 成线性关系); 假设 4 `11|b|a|2|d` and the grouping measure `11|b|a|d` rest on `10|a|a|7|a|5` (任职期内方差是转换风险的仿射变换). Both imports were `单卡` in round 1 and `整块` after round 2 (closures: 第 6 章's proof chain with `10|a|a|f`, `f|a`, the setting card, +7; 第 7 章's chain with `g`, `g|a`, `e`, `c`, the setting card, +10). The Model Card's **关联** `参照 [[10|a|a|3]]：本模型检验它的两条预测` (false share) now reads `参照 [[10|a]]：本模型检验它第 3 章与第 4 章的预测（经 **引入**）`.
- **13\|c, 13\|d, the transition process.** Their state measures (`13|c|a|b`, `13|d|a|b`) read the median split as the two states of 10\|a's variant two; each gained `依赖 [[10|a|a|e]]：好坏两个状态及其转移按学习模型变体二的两状态马尔可夫过程读（经 **引入**）`, and 13\|d's conjecture rests on `10|a|a|e` too. Import `整块 [[10|a|a|e]]` (closure: setting card and assumption card, +2). The Model Cards' Links to 10\|a's 第 4、5 章 (false shares) now name the Model Card: 13\|c `对应 [[10|a]]：两状态设定取自它的马尔可夫过程（变体二，经 **引入**）；它的第 5 章用同一组转移频率组织证据。`; 13\|d `参照 [[10|a]]：它的第 5 章…过程设定在变体二（经 **引入**）` and `参照 [[10|a]]：它的第 4 章…给出转换风险的来源`.
- **Decision: no import of 10\|a's 第 5 章.** The chapter's conclusion and example (`10|a|a|5|a|1`, `|1|a`) state Table 9 panel A and Table 10 numbers that 13\|c's description `13|c|a|c` and 13\|d's estimates also state: the formal model holds the §VI.B evidence. Importing the chapter into 13\|c or 13\|d would make those numbers members twice; importing 13\|c/13\|d parts into 10\|a would make the two sides lend to each other. The premise both tests use is the state process (`10|a|a|e`), which is imported. Left open (below).
- Symbols (step 6): every empirical model keeps its 变量表; the imported notation cards count as the models'; `check-settings` clean on all nine, no `沿用` line.

Import lines after the pass (text after `：` rewritten by hand to say what the model uses):

| Model | Lines | Members (main/side/shared/off) | Tolerated before → after |
|---|---|---|---|
| 7\|a | 整块 6\|a\|a\|2\|b | 77 (9/60/7/1) | 0 → 1 |
| 8\|a | 单卡 3\|a\|a\|1\|a\|1; 整块 6\|a\|a\|2\|b | 33 (6/18/8/1) | 0 → 2 |
| 9\|a | 单卡 3\|a\|a\|1\|a\|1; 整块 6\|a\|a\|2\|b | 35 (6/20/8/1) | 0 → 2 |
| 9\|b | 单卡 3\|a\|a\|1\|a\|1; 整块 6\|a\|a\|2\|b | 32 (7/16/8/1) | 0 → 2 |
| 11\|b | 单卡 10\|a\|a\|3\|a\|1, 10\|a\|a\|3\|a\|4, 10\|a\|a\|4\|a\|1; 整块 10\|a\|a\|6\|a\|4, 10\|a\|a\|7\|a\|5, 6\|a\|a\|2\|b | 79 (8/42/28/1) | 2 → 7 |
| 13\|a | 单卡 3\|a\|a\|2\|b; 整块 6\|a\|a\|2\|b | 39 (6/23/8/2) | 0 → 4 |
| 13\|c | 单卡 3\|a\|a\|1\|a\|1; 整块 6\|a\|a\|2\|b, 10\|a\|a\|e | 35 (6/17/11/1) | 2 → 2 |
| 13\|d | 单卡 3\|a\|a\|1\|a\|1; 整块 10\|a\|a\|e, 6\|a\|a\|2\|b | 33 (6/15/11/1) | 3 → 2 |
| 6\|a | 单卡 5\|a\|a\|2 | 12 (3/8/1/0) | 0 → 2 |

What stays tolerated: the premises of the `单卡` hypothesis imports (`3|a|a|1|a`; `3|a|a|2|a`, `|2|b|a`, `c`; 10\|a's `3|a`, `2|b`, `3|a|3`, `3|a|4|a|5`, `|a|6`, `4|a`), which a test does not take, and in every score user `5|a|a|2`, the premise of 6\|a's setting card that a `整块` import of 6\|a does not carry (a lender's own imports stay with the lender, as in ozhan2026monetary 13\|a).

### Cross-book blocks

None written. `plan` proposed none, and no part of the twelve models rests on another book's Card. The cross-book Links stay comparisons (grossman2002integration/4\|a\|a on 3\|a; assenza2026louder/5\|b\|c\|8 and atkinson2015lectures/14\|d\|a\|d\|1 on 10\|a; brandimarti2026reorganizing/6\|d\|a\|6 and hansen2021demand/1\|b\|10 on `3|a|a|1`; hansen2021demand/2\|a on `3|a|a|3`; gomezcram2026government/9\|a\|2 on `10|a|a|5`; liu2026managing/11\|a\|1 on 6\|a; goldsmithpinkham2020bartik/2\|c\|b\|3 on 11\|b; arrocannarsa2026flight/10\|i\|a\|7 on 13\|d), left for the evidence dialogue.

### Explanation refresh (step 8)

- `explain.mjs plan`: `nothing to explain` on all twelve heads (`parts: 0, jobs: []`). The Derivation parts of 10\|a (and 3\|a's, 6\|a's) already carry **思路**/**解读** whose position lines match their chains after stage 3 (checked by hand on `10|a|a|2|a|b`, `第 1 张，共 2 张`, and `10|a|a|7|a|3`, `第 2 张，共 3 张`); the empirical models have no `推导：估计式` part that lacks them. No estimate's premise changed its content (the new `依赖` lines name the theory a hypothesis or measure already named in words), so no **量级**/**为什么** was reread.
- No DeepSeek conversation was started: no job, no cost, nothing to delete and no DeepSeek workspace to remove. The twelve local work folders (`%TEMP%\zt-model-explain\bedford2026learning-*`) were deleted.
- `explain.mjs finish`: exit 0 on all twelve (`explained 0 part(s); Term links changed 0 Card(s), second plan 0; left 0; refused 0`).

### Gate

- `imports.mjs check`: exit 0 on all twelve. `plan`: `nothing to import` on eleven; on 6\|a it proposes `upgrade: 整块 [[5|a|a|2]]`, not applied (decision above).
- `get_model`: `self_sufficient: true` on all twelve; `false_share` empty on all twelve; no `closure`, `forward`, `reach_in`, `unanchored`, `import_target` or `import_cycle` fault; loose 0 on all twelve (3\|a 1 → 0, 13\|c 1 → 0). Members: 3\|a 16, 6\|a 12, 5\|a 22, 10\|a 62, and the empirical models as in the table.
- `check-settings.mjs`: 3\|a, 6\|a, 10\|a `settings: all in the setting part, as a dependency tree`; the nine empirical models `settings: the data tree and the specification ladder are in place`.
- `check-network.mjs`: `network: clean` on all twelve.
- `explain.mjs finish`: exit 0 on all twelve.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"cards_refused":0,"cards_changed":16,"links_repaired":0,"rounds":1,"applied":true,"book":"bedford2026learning","links_added":32,"ok":true}`. `check`, `get_model` and the comparison below were rerun after it and stay as listed.
- Whole-book comparison with the backup (Term macros stripped): 867 Cards before and after; 57 Cards changed (the 34 hand-written Cards, the 9 Model Cards with import lines among them, plus lender and Term Cards whose regenerated back-link lists changed; no other text changed). Display formulas 1,194 → 1,191 (distinct 1,087 → 1,085): lost exactly the three restated copies above (`10|a|a|e` (C2), `10|a|a|g` (C2), `10|a|a|c` (C4)), nothing added. Equation tags 139 → 136: lost `\tag{C2}`, `\quad(C2)`, `\quad(C4)` with those displays; (C2) and (C4) still stand in the setting card. Estimate sections (**估计**/**值**/**量级**) 182 → 182, identical.

### Open

1. 10\|a's 第 5 章 (`10|a|a|5`, §VI.B) holds evidence: its conclusion `5|a|1` and example `5|a|1|a` state the Table 9 panel A frequencies that 13\|c's description `13|c|a|c` states, the friction null of 13\|c and the Table 10 rates of 13\|d. A later pass could make the chapter rest on those parts (a formal model citing estimates imports them `单卡`) or move the numbers to the empirical models; no import was written because either direction would make the models lend to each other.
2. 6\|a imports `5|a|a|2` `单卡` by decision (tolerated 2; `plan` keeps proposing the `整块` upgrade). Every score user therefore tolerates `5|a|a|2` through 6\|a. If the owner wants tolerated 0, either 6\|a takes 5\|a's specification block whole or the eight models add `单卡 [[5|a|a|2]]`, which they do not use.
3. (C5)(C6) still stand in two cards of 10\|a (setting card: the defining identities; assumption card: the ordering and $\lambda>0$); each holds content the other lacks.
4. Copy advice left as judged comparisons: 3\|a\|a\|1 ~ 10\|a\|a\|1, 10\|a\|a\|4; 3\|a\|a\|3 ~ 10\|a\|a\|4, 10\|a\|a\|7; 10\|a\|a\|4 ~ 11\|b\|a\|7. They come from `对应` text in old **关联** paragraphs of the chapter cards (stage-3 open item 5).
5. For stage 5 (illustrate the formal models 3\|a, 6\|a, 10\|a): the blocks they lend are their own parts and are illustrated in the home model; 6\|a's setting card now rests on 5\|a's equation (2), which a figure of the setting can show as an imported input. 10\|a's variant two and variance mapping now point to the setting card's (C2) instead of writing it.
6. For stage 6 (evidence dialogue of the nine empirical models): the hypotheses now rest on imported results, so 印证/反驳 Links from estimates to hypotheses can name the theory's result through the import; the cross-book comparisons listed above wait for it.
7. Stage-3 open items 4–5 stay. Tree unlocked throughout; nothing committed (stage brief).

## Stage 5: illustrate

Date 2026-11-21, live box (owner asleep, overnight authority: every decision below was taken without asking and is recorded here), under `zt-model-illustrate`, done in one side session by scripts. No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-bedford2026learning-illustrate.sqlite3` (`VACUUM INTO` through `node:sqlite` read-only, 521 MB, `PRAGMA integrity_check` ok; drive D had 198 GB free). Only the three formal models were illustrated, each finished and written before the next: 3\|a, then 6\|a, then 10\|a. The nine empirical models have no figures (owner, 2026-11-11). Blocks the three models lend to the empirical models are their own parts and were illustrated here; the imported `5|a|a|2` (equation (2), `单卡` in 6\|a) belongs to 5\|a and got no figure or **直觉** (verify skips it). Modules (uncommitted, under `~/.claude`):

- `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\bedford2026learning-3_a.mjs`
- `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\bedford2026learning-6_a.mjs`
- `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\bedford2026learning-10_a.mjs`

Before each live write, every figure was rendered locally from the module (a copy of `verify.mjs` that reads the figures from the module, session scratchpad `ill/local.mjs`) in light and dark, one PNG per part, and the PNGs were inspected. `apply.mjs --write` then wrote 5 Cards (3\|a: the Model Card and 4 setting cards), 2 Cards (6\|a: the Model Card and the setting card) and 11 Cards (10\|a: the Model Card, 9 setting cards, the 定义 part `7|a|1`), each `errors 0`. Two later writes changed one **直觉** line each (see Faults). No display formula, equation number or text outside the chart and the **直觉** section changed (apply guard). Every Model Card figure has three parts (`part()`, top-level `data-part`).

### Figures

- Model Card 3\|a (**图**, before **结构**): (一 · 次序与参与者) five panels: nature (binary $\theta_t$, Markov evolution), frictions ($\mathcal{C}(\mathcal{F})$ and the precision of $\mathcal{I}_0$), the board's choice (1), each period after appointment (board: public + private signals, retain or dismiss; market: public signals, price), state transitions; the solution order 第 1 章 → 第 2 章 → 第 3 章. (二 · 因果结构) frictions, prior and the state process → channel one $\mathcal{C}(\mathcal{F})$ and channel two (precision of $\mathcal{I}_0$) → the board's $c^{*}$ (1) → $E[\theta]$ → post-appointment signals → board and market posteriors; dashed: screening into channel two, switching risk back into the beliefs. (三 · 框架研究的变化) six panels: the two channels and the uninformative limit (第 2 章), switching risk and noisier pre-appointment signals (第 3 章), the three testable directions (第 1 章).
- 3\|a setting cards (4), at the end of **说明**: `3|a|a` (the two states $G$, $B$ behind a veil with stay/deteriorate/improve arrows; frictions cutting the feasible set out of all candidates and setting the precision of $\mathcal{I}_0(c,\mathcal{F})$; the prior $p_0$ on the unit interval, 示意); b (board, CEO, investors; a 2 × 2 CEO × firm table of fit, 示意; the information timeline: pre-appointment signal, appointment, per-period performance signals); c (TFP as portable ability + firm fixed part + match-specific part, the board looks at the last, 示意; search and attraction frictions → channel one, screening → channel two, no third channel); d (public and private signals into the board, public only into the market; retain or dismiss against price; the per-period loop observe → update → act).
- Model Card 6\|a: (一 · 构造次序) six panels: data and the same-year comparison set, program (3)–(6), the directions, relative efficiency (7), stage one, stage two; solution order 第 1 章 → 第 2 章. (二 · 成分结构) the bar of equation (2)'s components ($\alpha_t$, $\delta_j$, $\eta_m$, $\xi_{mj}$, noise; each opens `5|a|a|2`), stage one removing $\alpha_t$, stage two removing $\delta_j$, the remainder ($\eta_m$, $\xi_{mj}$, noise) → the match-quality proxy. (三 · 性质与选择) scale invariance, DDF against input-oriented DEA, (7), the yearly frontier, the firm fixed-effect residualization (第 1–2 章), and the imported equation (2) as its own panel. Since 6\|a has no players, part one is the order of computation, not a game.
- 6\|a setting card `6|a|a`: one unit $j$ (inputs $x_{1j}$ … $x_{mj}$ → operating profit $y_{rj}$, possibly negative; focal unit $o$); the same-year comparison sets, one per year; the cone combination $\lambda_1$·unit 1 + $\lambda_2$·unit 2 with $\lambda_j\ge 0$ in the input–profit plane, a negative-profit unit, 示意; **the imported decomposition (2) as a dashed link-coloured box that opens `5|a|a|2`** (`引入，单卡`), with the four symbols named.
- Model Card 10\|a: (一 · 次序与参与者) six panels: appointment and prior, the period-start transition (variant two) or $\tilde p_t=p_{t-1}$ (variant one), the two signals, the update (9), risk-neutral pricing, the researcher's noisy measure; solution order 第 1–7 章, and the note that the market only learns. (二 · 因果结构) true type, signal parameters, prior and the transition matrix (C2) → signals (two log-likelihood ratios) → filtered belief $p_t$ (9) → prediction $\tilde p_{t+1}$; $p_t$ → uncertainty $p(1-p)$ → return variance (pricing); (C2) → state path $Z_t$ → measured within-tenure variance; dashed: the prediction is next period's start. (三 · 模型研究的变化) nine panels: event information content (第 2 章); convergence, the jump $J$ and drift $D$, asymmetric learning (第 3 章); the positive lower bound and the more persistent good match (第 4 章); the two-state persistence (第 5 章); return variance linear in $p(1-p)$ (第 6 章); within-tenure variance affine in transition risk (第 7 章).
- 10\|a setting cards (9): `10|a|a` (hidden type and the two signals into the market; the one-period order $p_0=\tilde p_1$ → $\tilde p_t$ → observe $y_t$, $X_t$ → $p_t$ → next period; $y_t=\mu_\theta+\varepsilon_t$ as mean + noise; $q_\theta(t)=q_{\theta,0}e^{-\lambda(t-1)}$ over tenure, 示意; the transition matrix (C2) as a 2 × 2 table); b (one inferrer, the prior, one signal a period; the fixed and the time-varying case); c (the two conditional densities with $\mu_G>\mu_B$ and a common variance, 示意; $q_G(t)\gg q_B(t)$ decaying with $\lambda>0$, 示意; the conditional-independence graph $\theta_t\to y_t$, $\theta_t\to X_t$, no edge between them); d (two whole tenures, all $G$ or all $B$; (C2) with $P_{G\to G}=1$, $P_{B\to G}=0$; $p_{t-1}$ → no transition → $\tilde p_t$); e (the two-state switching diagram with all four probabilities; the per-period order transition → predict → observe → update; the prediction step as $p_t\times P_{G\to G}$ plus $(1-p_t)\times P_{B\to G}$); f ($\mathcal{I}_t$ → $\theta_{t+1}=G$ with $p$, $B$ with $1-p$ → $\mu_G$ or $\mu_B$, plus noise → $x_{t+1}$); f\|a (the two dates of the one-shot asset, price (C22) and payoff with zero continuation value; risk neutrality, $R>1$, the gross return (C23)); g (a ten-year state path with $Z_{it}$ under it, 示意; $M_{it}=\mu_B+\Delta Z_{it}+\varepsilon_{it}$ as points around the two dashed levels $\mu_B$, $\mu_G$ in the plane tenure year × measured match quality, 示意); g\|a (five identical yearly error densities, independence from the state path, $\Delta>0$ on a number line).

### 直觉

The **直觉** section was rewritten on all 14 setting cards (3\|a: 4; 6\|a: 1; 10\|a: 9) and on the one 定义 part of 10\|a (`10|a|a|7|a|1`; no other chapter card of the three models has facet `定义`). Every one had a digest or stage-2 section. The old sections named cards or facets (`规划卡`, `效率卡`, `机制一节`, `结论一节`, `定义卡`, `命题卡`, `推导：测得方差的分解`), named later results or equations ((C11), the asymmetric-learning proposition, the bound of $\tilde p_{t+1}$ in `e`, the three directions in 3\|a\|a\|b) and carried `（补：书中未写）` (10\|a\|a\|b). The new sections name chapters in words, with no Link, no result, no 依赖 line and no assumption name; each uses $TeX$ only for symbols its card or an earlier setting defines.

### Chart fixes outside the setting part

- `10|a|a` (setting card) held a digest chart inside **设定** item 3 (the one-period timeline, raw `p0`, `p̃_t`, `y_t`, own marker id). `apply.mjs` replaces a setting card's earlier charts, so it went; the new figure at the end of **说明** keeps that timeline as its second block. Item 3 still says `下图给出一期内的时序`; the guard forbids changing that sentence, and the figure stands further down the same card.
- Four digest charts in chapter parts of 10\|a failed lint (`svg-math`, `axis-label`) and were fixed in place by a scratchpad script that changes text inside the chart fence only and refuses any change outside it (`ill/fixcharts.mjs`, spec `ill/spec10.mjs`), rendered light and dark before the write, then written (`written 4, errors 0`): `2|b` (subscripts in `信念 p_t`, `不确定性 p_t(1−p_t)` as tspans; the four axis labels tagged), `3|a|4` (the y label `p_t` at mid-height became `信念 p_t` over the axis; both axes tagged), `4|a|1|a` (axis labels `任期 t`, `关于类型的不确定性`; the curve labels `初始强匹配`, `初始弱匹配` moved off the curves), `7|a|a` (axis labels `好匹配年份的占比 Z̄`, `二值方差 Z̄(1−Z̄)`). Curves, points and every other text unchanged.

### Faults met in the local renders and the checks

- Local renders: the label `候选人 c` over a candidate circle and a dashed precision arrow through the candidate pool (3\|a\|a); the screening label hidden behind the channel-one box and a dismissal loop drawn through both channel boxes (3\|a causal; the loop "解雇后重新选人" was also not in the source, so it was dropped); the causal viewBox 775 wide (lint `width`, made 760); the benchmark label over unit 1 and the section heading on the y-axis label (6\|a\|a); the nested subscript `μ_{θ_t}` rendered raw (10\|a\|a: written `μ_θ`, with `θ = θ_t` in the heading); headings on the y-axis labels (10\|a\|a event plot, 10\|a\|a\|g); crossing arrows through the belief box in the first 10\|a causal layout (re-laid out in four columns).
- A headless-Edge artifact: the solo dark PNG of 3\|a part two shows a stray vertical dashed line under the panel, below the svg; the full dark page, the light solo page and the verify render of the full card are clean. Moving and reshaping the path moved the artifact with it, so it is a screenshot artifact of the solo view, not a figure fault; left as is.
- `check-premises` on 3\|a after the first write: the new **直觉** of `3|a|a|c` named `(1)` (derived later by `3|a|a|2|a`); rewritten to "董事会的选择规则", then 0 hits.
- `zt-check` on `10|a|a|c`: R4 in the new 为什么需要 line (`会被重复计算`); rewritten, 0 faults.

### Gate

All lines below are the reruns after the Term relink.

- `verify.mjs --model bedford2026learning/3|a` (light and dark): `5 chart(s) in 17 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets.
- `verify.mjs --model bedford2026learning/6|a`: `2 chart(s) in 13 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets.
- `verify.mjs --model bedford2026learning/10|a`: `14 chart(s) in 63 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets. Before this stage: `5 chart(s); 11 lint fault(s); 9 card(s) without a figure`.
- `apply.mjs` rerun: `same` for all 5 + 2 + 11 Cards.
- `check-settings.mjs`: 3\|a, 6\|a, 10\|a `settings: all in the setting part, as a dependency tree` (10\|a: the four `uses an assumption` advice lines as at stage 4).
- `check-model.mjs`: 3\|a `relations {"refers":15,"rests-on":18,"illustrates":2,"corresponds":2}; loose 0; self_sufficient true`; 6\|a `relations {"rests-on":11,"refers":10,"contrasts":1}; loose 0; self_sufficient true`; 10\|a `relations {"refers":64,"rests-on":85,"contrasts":4,"corresponds":13,"illustrates":2}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` on all three (3\|a advice: chapter 3 has no `结果：` card, as before).
- `check-premises.mjs`: 3\|a and 6\|a `premises: none used before their card`; 10\|a 2 hits, both existing pointers to (9) in the **说明** of `10|a|a|b` and `10|a|a|e` (stage-4 text; a pointer stays).
- `zt-check --cards` on the written Cards: 3\|a 2 faults (R3 in `3|a|a|b` **说明** and `3|a|a|c` **假设**), 6\|a 1 (R3 in the Model Card's stage-4 import line), 10\|a 7 (R13 on `10|a|a` 33 sentences and `10|a|a|e` 28; R3 in `10|a|a|b` **说明**, `e` **起点**/**设定**/**说明**, `f` **设定**). All in existing text; none in a **直觉** section or a chart.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"cards_refused":0,"book":"bedford2026learning","ok":true,"links_added":13,"applied":true,"links_repaired":0,"rounds":1,"cards_changed":6}`; `-PlanOnly` afterwards: `links_added 0, cards_changed 0`.

### Open

1. Stage 7 (literature organize) will renumber addresses. zt 0.63+ rewrites chart `data-zt` targets with the moves, but each module's `head` (`bedford2026learning/3|a`, `6|a`, `10|a`, and the constant `EQ2 = 'bedford2026learning/5|a|a|2'` in the 6\|a module) must be changed to the new addresses after the organize; then run `apply.mjs --write` and `verify.mjs --model` on each new head (0 dead targets, `same` on a second `apply.mjs`).
2. `10|a|a` **设定** item 3 still says `下图给出一期内的时序` although the timeline now stands in the figure at the end of **说明** (the guard keeps text outside the chart). A one-word change (`下图` → `卡末的图`) waits for the owner or a later text pass.
3. The 10 zt-check faults (R3/R13 in existing **起点**/**设定**/**假设**/**说明** text and the import line) predate this pass and were not changed.
4. Several figures draw shapes or values the source does not fix (the candidate pool, the CEO × firm fit table, the TFP component lengths, the cone points, the event-probability curves, the conditional densities, the ten-year state path and its measured points, the component bar of (2)); each caption says 示意.
5. The stage-4 open items stay. No `card_locked`, `scope_refused` or `read_view_changed` error and no failed write. Nothing committed (stage brief).

After the stage (main session): the open sentence in `10|a|a` **设定** item 3 now reads "**说明**末尾的图给出一期内的时序" (was "下图…"), written by script and read back the same.

## Stage 6: evidence dialogue

Date 2026-11-21, live box (owner asleep, overnight authority: every decision below was taken without asking and is recorded here), under `zt-evidence-dialogue` steps 0–5 for the nine empirical models, one at a time in the order 5\|a, 7\|a, 8\|a, 9\|a, 9\|b, 11\|b, 13\|a, 13\|c, 13\|d (each surveyed after the previous one was applied, so the judged record dropped the mirror pairs). 3\|a, 6\|a and 10\|a are formal and have no dialogue. Backup: `D:\zt\backups\zt-before-bedford2026learning-dialogue.sqlite3` (`VACUUM INTO` through `node:sqlite` read-only, 521 MB, `PRAGMA integrity_check` ok). Step 0: the Term relink before the first survey changed nothing (`applied: false`, `links_added: 0`). Out folders `D:\zt-dialogue\bedford2026learning\<5a|7a|…>`. No Card was written.

### Survey

| Head | Ours | Theirs | Pairs | Messages | Our estimates paired | Other side (pairs) |
|---|---|---|---|---|---|---|
| 5\|a | 6 | 1,616 | 375 | 32 | 5 | 7\|a 135, 8\|a 30, 9\|a 30, 11\|b 75, 13\|a 70, 13\|c 35 |
| 7\|a | 27 | 1,595 | 49 | 5 | 1 (`7|a|a|7|c`, 0.224) | 5\|a 1, 8\|a 6, 9\|a 6, 11\|b 15, 13\|a 14, 13\|c 7 |
| 8\|a | 6 | 1,616 | 438 | 37 | 6 | 5\|a 6, 7\|a 156, 9\|a 36, 9\|b 24, 11\|b 90, 13\|a 84, 13\|c 42 |
| 9\|a | 6 | 1,616 | 402 | 34 | 6 | 5\|a 6, 7\|a 156, 9\|b 24, 11\|b 90, 13\|a 84, 13\|c 42 |
| 9\|b | 4 | 1,618 | 0 | 0 | 0 | none |
| 11\|b | 15 | 1,607 | 528 | 44 | 11 | 5\|a 11, 7\|a 286, 13\|a 154, 13\|c 77 |
| 13\|a | 14 | 1,608 | 195 | 17 | 8 | 5\|a 5, 7\|a 130, 11\|b 20, 13\|c 38, 13\|d 2 |
| 13\|c | 7 | 1,615 | 160 | 14 | 4 | 5\|a 4, 7\|a 104, 11\|b 16, 13\|a 36 |
| 13\|d | 6 | 1,616 | 0 | 0 | 0 | none |

- **All 2,147 pairs are within the book** (estimate against estimate of another bedford2026learning empirical model); none is a Finding Card or another book's estimate. The estimand Terms are the book's own (匹配质量 `1|a|a` in every model but 9\|b and 13\|d, 匹配效应 `1|a|h`, 管理者效应 `1|a|i`, 混合效应模型 `1|a|y`, 强制 CEO 更替 `1|a|ar`, 股票收益 `1|a|at`, 任期 `1|a|bh`, 好匹配/坏匹配 `1|a|bx`/`1|a|by`, …, and the book root, which forms no pair). None of them links to a Term Card of another book, so zt-term-sync has widened nothing.
- **Survey noise.** Most estimates of 5\|a, 8\|a, 9\|a, 11\|b, 13\|a and 13\|c carry no Term of what they measure beyond the estimand Terms, so by the script's rule they pair on 匹配质量 against every estimate of the book that names it (for example 11\|b pairs 286 times with 7\|a). The asymmetry 7\|a → 49 against 8\|a → 7\|a 156 is that rule: 7\|a's estimates link their own regressors.
- **Hand search for cross-book neighbours** (`search_cards`: Finding Cards on CEO and 更替; all Cards on 管理者效应; Model Cards on 管理者). Only bedford2026learning and hansen2021demand have CEO Finding Cards. hansen2021demand estimates the demand for executive social skills in search mandates (`5|c|e`, `1|c`, `7|a|b`: CEO searches are 42.8% of five titles), not a match or manager variance component, a turnover probability or a return response: not the same estimand. No other book states a forced-turnover or announcement-return estimate. brandimarti2026reorganizing has no Card in these searches. No cross-book pair was forced.

### Judging (DeepSeek)

One conversation per model with pairs (seven conversations), `opencode-go/deepseek-v4.1-flash`, effort high, OpenCode Go account 904315849\_002 (workspace 002), one job at a time; the official source was not used (its ledger line stayed at 16.0161 CNY, 1,797 calls throughout). Prompts were sent as `prompt_files`: `references/judge.zh.md` + `judge-1.md`, then each later file in the same conversation, one message at a time. No 429, no tool call, no `needs_response`, no resend. Conversations: 5\|a 88a643ce (32 turns, context 294k of 1M at the end), 7\|a 380634fc (5 turns, 73k), 8\|a 828ca91b (37, 340k), 9\|a 982aea24 (34, 316k), 11\|b dcfa0aa1 (44, 398k), 13\|a 3e29bfab (17, 167k), 13\|c 8008d57b (14, 147k). Each was deleted after its answers were collected; the workspace `D:\zt-dialogue\bedford2026learning` was removed at the end.

- **Decision: answers collected by script, not by hand.** The judge's JSON lines were read from the conversation's stored session (`~/.dsh/sessions/--D-zt-dialogue-bedford2026learning--/<id>/session.v4.jsonl.zstd`, zstd frames decoded with `node:zlib`) by `D:\zt-dialogue\bedford2026learning\extract-answers.mjs` before the delete; every model's `answers.jsonl` holds exactly one line per pair (`missing: []` on all seven). The message loops of 5\|a (files 2–32), 8\|a, 9\|a, 11\|b, 13\|a and 13\|c were driven by a general-purpose helper agent that only sent the files; 7\|a was sent from the main session. The helper for 8\|a sent `judge-5` and `judge-6` in one step (briefly two requests in flight in one conversation); both answers came back complete and in order.
- **Usage.** The opencode-go ledger moved by exactly 151 calls between the snapshot after 5\|a and the end, the number of requests of the six later conversations (5 + 37 + 34 + 44 + 17 + 14), so no other session wrote in that window: 1,442,272 uncached input, 25,034,624 cached, 158,937 output tokens (cache hit about 95%). 5\|a's 32 calls were not isolated (no snapshot before it); by its context growth they are of the same order. Cost 0 CNY (OpenCode Go subscription).
- **Verdicts.** All 2,147 answers came back `same: false` (`not-comparable`, `kind: none`). Checked by reading the reasons of the within-book pairs most likely to share an estimand: 9\|a's return-decile coefficients against 9\|b's announcement-day abnormal returns (different event and window), 13\|a's appointment-friction coefficients on initial match quality against 13\|c's coefficients on state persistence and transition (different dependent variable; the judge named it, for example `13|a|a|4|e` −3.623 against `13|c|a|4|d` −0.248), 13\|a against 13\|d's shock-response differences, and 5\|a's variance components against 7\|a's standardized coefficients (variance against a regression coefficient): right.

### Pairs judged and written

| Head | Pairs | Same estimand | Agree / disagree / not comparable | Links written |
|---|---|---|---|---|
| 5\|a | 375 | 0 | 0 / 0 / 0 | 0 |
| 7\|a | 49 | 0 | 0 / 0 / 0 | 0 |
| 8\|a | 438 | 0 | 0 / 0 / 0 | 0 |
| 9\|a | 402 | 0 | 0 / 0 / 0 | 0 |
| 9\|b | 0 | 0 | 0 / 0 / 0 | 0 |
| 11\|b | 528 | 0 | 0 / 0 / 0 | 0 |
| 13\|a | 195 | 0 | 0 / 0 / 0 | 0 |
| 13\|c | 160 | 0 | 0 / 0 / 0 | 0 |
| 13\|d | 0 | 0 | 0 / 0 / 0 | 0 |

`apply` on the seven heads with pairs: `{"links":0,"written":0,"errors":0}` each. No apply on 9\|b and 13\|d (no pair). All 2,147 answered pairs are in the judged record (`~/.zt-evidence-dialogue/judged-default.jsonl`, 2,147 lines naming bedford2026learning), so no later survey pays for them again. The **对应** lines already on the estimates (印证 to the book's own hypotheses, written in stages 3–4) stay as written.

### Gate

- `dialogue.mjs check`: `{"broken":0,"proposed_again":0,"ok":true}` (exit 0) on all nine heads (5\|a, 7\|a, 8\|a, 9\|a, 9\|b, 11\|b, 13\|a, 13\|c, 13\|d).
- No same-estimand pair, so no new **对应** line to check.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink after the stage: `{"applied":false,"rounds":0,"cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"ok":true}`.

### Open

1. No cross-book pair: the book's Terms (匹配质量, 管理者效应, 强制 CEO 更替, …) link to no Term Card of another book. After a zt-term-sync pass on the economics shelf (and after stage 7, at the new addresses), survey again; the judged record keeps the 2,147 pairs from being judged twice. hansen2021demand stays a different estimand.
2. Survey noise: estimates without a Term of what they measure pair with every estimate naming 匹配质量 (2,147 pairs for 0 Links). A Term link for each estimate's own regressor or outcome would narrow a later survey; not added here (the stage writes only judged Links).
3. The stage-4 open item 1 stays: 10\|a's 第 5 章 states the Table 9/10 numbers that 13\|c and 13\|d also state. 10\|a is formal, so the survey does not read it; the overlap is a question for an import or a text pass, not a dialogue Link.
4. For stage 7 (literature organize): the dialogue wrote nothing, so it adds no address the organize must carry; the stage-5 open item 1 (chart module heads to change after the renumbering) stays.
5. No `card_locked`, `scope_refused` or `read_view_changed` error and no failed write. No skill or script changed. Nothing committed (stage brief).

## Stage 7: literature organize

Date 2026-11-21, live box (owner asleep, overnight authority: every decision below was taken without asking and is recorded here), under `zt-literature-organize`, done in one side session by scripts (`outline.mjs`, seven structure batches through `batch.mjs`, one `leads.mjs` run, one scripted `edit_card` batch, one root/structure write). No subagent and no DeepSeek job ran (the summaries were written by hand from the Cards under each division), so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-bedford2026learning-organize.sqlite3` (`VACUUM INTO` through `node:sqlite` read-only, 521 MB, `PRAGMA integrity_check` ok). Every batch was previewed first (complete true, refusals none, `links_that_will_break` empty) and committed under its plan hash. The tree went from 868 to 872 Cards (four new summary leads: §II, §III, §IV, Appendix C). No Card was deleted. The text batch compared the display and inline formulas and `\tag{}` of each old and new body: 0 formulas added or changed, 0 removed (the trimmed heads keep `$[-20, 20]$` and `$\tilde p_{t+1}(1-\tilde p_{t+1})$`); no model part was edited. Only Cards of bedford2026learning were written. No `card_locked`, `scope_refused` or `read_view_changed` error; the tree was unlocked throughout.

### Source divisions

From the bilingual file 2408: title page with authors, abstract and acknowledgement footnote; I. 引言; II. 管理者—企业匹配质量：概念框架 (A. 相关文献; B. 概念框架 with 1. 潜在的、不断演变的状态, 2. 摩擦下的聘用决策, 3. 任命后的学习与决策, 4. 与经验分析的对应); III. 相关性与度量 (A–D); IV. 后果 (A–C); V. 资本市场学习 (A–B); VI. 任命环节与状态转移中的摩擦 (A–B); VII. 结论; 参考文献; 附录 A; 附录 B; 附录 C (A–F, Figure C1 at its end); then 表 1–10 and 图 1–10 with no heading of their own. The top line takes the title page, the seven sections, the three appendices and the two blocks of tables and figures, then the Term list and the Cited list (the source's 参考文献 is the Cited list at the tail).

The digest had filed the paper as 20 groups: old 1 introduction with the Term list, the Cited list, the abstract card 1\|e and the structure Card 1\|j under it; old 2 II.A; old 3 II.B (the framework model 3\|a); old 4 II.B.3 + B.4; old 5–7 III.A–C; old 8 III.D + the §IV preamble + IV.A; old 9 IV.B + IV.C; old 10 §V; old 11 V.A; old 12 V.B; old 13 §VI; old 14 VII + Appendix A; old 15 Appendix B; old 16–20 Appendix C.A, C.B, C.C + D, C.E, C.F (old 20 also held the table and figure cards 20\|b, 20\|c).

### Final tree (top line and divisions)

```
1   标题页与摘要 (old 1|e)                      a 结构卡 (old 1|j)
2   I. 引言 (old 1)                            f §I chain, g–i findings
3   II. 概念框架 (new, demote of old 2)        a A. 相关文献 (old 2, chain old 2|a)
                                              b B. 概念框架 (old 3, rewrite): a framework model (old 3|a) · b 3. (old 4|b..b|1) · c 4. (old 4, trimmed; chain old 4|b|2..4)
4   III. 相关性与度量 (new, demote of old 5)    a A (old 5, model old 5|a) · b B (old 6, model old 6|a) · c C (old 7, model old 7|a)
                                              d D (old 8|b..b|3, findings old 8|c,d,e,g,h,i)
5   IV. 后果 (new, demote of old 8)            a §IV preamble (old 8|b|4) · b A (old 8, trimmed; model old 8|a, chain old 8|b|5..7, findings)
                                              c B (old 9|c..c|1, model old 9|a, findings old 9|d,g) · d C (old 9, trimmed; model old 9|b, chain old 9|c|2..5, findings)
6   V. 资本市场学习 (old 10, rewrite)           a learning model (old 10|a) · b §V chain (old 10|b) · c A (old 11) · d B (old 12)
7   VI. 摩擦 (old 13, rewrite)                 a §VI preamble (old 13|e) · b A (old 13|e|1..6, model old 13|a, findings) · c B (old 13|e|7..11, models old 13|c, 13|d, findings)
8   VII. 结论 (old 14|b, chain as 8|a)
9   附录 A (old 14, trimmed)                   b chain (old 14|b|3..7), c–f findings
10  附录 B (old 15)
11  附录 C (new, demote of old 16)             a A (old 16) · b B (old 17) · c C (old 18|b..b|2) · d D (old 18, trimmed; chain old 18|b|3..6) · e E (old 19) · f F (old 20, with the Figure C1 panels old 20|e–g)
12  表 1 至表 10 (old 20|b)
13  图 1 至图 10 (old 20|c)
14  术语表 (Term list, from 1|a)               160 Cards
15  参考文献 (Cited list, from 1|b)            69 Cards
```

Batches, tail to head: b1 appendix C, tables and figures, VII out of old 14 (59 renamed); b2 §VI (110); b3 §V (248); b4 §IV with III.D to the III.A head (427); b5 §III (544); b6 §II (609); b7 VII chain under its head, both lists to the tail, the abstract to the top, the structure Card under it (867). One reorder in b2 (13\|e\|1, 13\|e\|7) and one in b3 (model 10\|a to the end) were dropped: the first was already the order (`side_of` appends in order), the second was a choice reversed (see Calls).

### Crossers and how each was settled

- Old 1 held the introduction, the abstract, the structure Card and both lists: the lists went to the tail (`14`, `15`); the abstract card became the first top Card `1` (title 标题页与摘要, source line `标题页与摘要`, was `I`) with the structure Card as `1|a`; old 1 is `2` (I).
- Old 4 held B.3 (two Cards) and B.4 (three Cards): the head went to B.4 by count and was trimmed (`3|b|c`, title `4. 与经验分析的对应：…`; the B.3 sentences and the `§3` index line dropped, they stand in `3|b|b` and `3|b|b|1`); the B.3 chain became `3|b|b`.
- Old 8 held III.D (four notes, six findings) and the §IV preamble + IV.A (four notes, two findings, the model): the head went to IV.A by count (the model) and was trimmed (`5|b`; the III.D and §IV sentences, the `§D`/`§IV` index lines and the `对应 …4|d|3` line dropped). The III.D chain became the III.D head `4|d` with its six findings; the §IV preamble `8|b|4` became `5|a`, placed before IV.A by the demote's `first`.
- Old 9 held IV.B (two notes, the model 9\|a, two findings) and IV.C (five notes, the model 9\|b, four findings): the head went to IV.C by count and was trimmed (`5|d`; the IV.B sentence and index line dropped); IV.B got the head `9|c` (`5|c`) with its chain, model and findings.
- Old 13 held §VI preamble, VI.A and VI.B on one Direct chain with all models and findings flat beside it: old 13 was rewritten as the §VI summary; the chain was cut into `7|a` (preamble), `7|b` (A, from 13\|e\|1) and `7|c` (B, from 13\|e\|7), each with its model(s) and findings.
- Old 14 held VII (three notes) and Appendix A (six notes, four findings): the head went to Appendix A by count and was trimmed (`9`; the VII sentences, the `§VII` index line, the old position line and the `承接 …8|a` line dropped); VII became the top Card `8` (old 14\|b) with its chain as `8|a`.
- Old 18 held C.C (three notes) and C.D (four notes): the head went to C.D and was trimmed (`11|d`, the C.C formula kept as one sentence naming C 节); the C.C chain became `11|c`.
- Old 20 (C.F) held the table and figure cards: they became the top Cards `12` and `13` (titles 表 1 至表 10 / 图 1 至图 10, source lines `表 1–10` / `图 1–10`, were `§F`).
- Whole units: each model moved once with all its parts (check.mjs: every unit the same size before and after). Term list 160 and Cited list 69 Cards, unchanged.

Calls that could go the other way: B.1 and B.2 have no reading note of their own (their content is the framework model's setting), so the II.B summary indexes both by the Model Card `3|b|a`; the §V learning model `6|a` stays the first Side branch of the §V summary (the book's own pattern: model first in its division) rather than last; the Figure C1 panel cards `11|f|e`–`g` stay under C.F, where the source prints the figure and where the C.F head describes it, although the figure shows the C.D simulations; the tables and the figures are two top Cards in the source's order, not one 表与图 Card; VI.A and VI.B open on their first reading note (no lead of their own), as do III.D, IV.B and C.C.

### Summary leads and other text

- `leads.mjs`: demotes `3` (§II), `4` (§III), `5` (§IV, `first` the §IV preamble), `11` (Appendix C); rewrites `3|b` (II.B), `6` (§V), `7` (§VI). Each has the source's name and main claim as title, one or two paragraphs that walk the subdivisions, `小节索引：` with a Link to each subdivision head, `在全书中的位置：` and the source line. leads.mjs turned the Term names into Term uses.
- Text batch (63 `edit_card` writes, 0 errors): titles with the section label on `1`, `2` (I.), `3|a` (A.), `4|a`–`4|d` (A.–D.), `5|b`–`5|d` (A.–C.), `6|c`, `6|d`, `7|b`, `7|c`, `8` (VII.), `9`, `10`, `11|a`, `11|b`, `11|d`, `11|f`, `12`, `13`; trimmed heads `3|b|c`, `5|b`, `5|d`, `9`, `11|d`; position lines made true in `1`, `2`, `3|a`, `8`, `9` and the II.B summary's 上一节 Link (leads.mjs had read `3|a` as the old chain card; now the II.A head); Links that meant another division: `4|c` IV → `5`, its 附录 A Link (it named Appendix B) → `9`; `6|d` and `11|a` C.C → `11|c`; `11|b` 下一节 → `11|c`; `11|f` now names `6|d` and `7` (it named Appendix A). Source lines added where a head had none (`2`, `3|a`, `3|b|c`, `4|a`, `5|b`, `5|d`, `9`, `10`, `11|a`, `11|b`, `11|d`, `11|f`). 40 Finding Cards named their old group head as source Link (zt-check F2 after the moves); they now link the book root with the label (`来源：[[bedford2026learning]] §III.A`, …, `§D` written `§III.D`).
- The root index lists the 13 content top Cards and the two lists (the old index named 20 digest groups). The structure Card `1|a` lists the top line and the 12 models at their new addresses (its old model list named chapters and parts of 3\|a and 10\|a as models, stale since the fold).
- `zt-check --cards` on the root, the structure Card, the 7 leads and the 63 written Cards: first run `faults: 16` (R2/R3 in new lines of the root and `1|a`), fixed twice (one R5 met on the way); last run on the two `faults: 0`; the other Cards had no fault (advisory L1 lines only).

### Chart targets

In `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\bedford2026learning-3_a.mjs`, `-6_a.mjs` and `-10_a.mjs` the `head` is now `bedford2026learning/3|b|a`, `4|b|a` and `6|a`, and in the 6\|a module `EQ2 = 'bedford2026learning/4|a|a|a|2'`, each with a comment naming the old address. The file names stay. `apply.mjs`: `same` on all 5 + 2 + 11 Cards (zt had rewritten every `data-zt` target with the moves), so nothing was written. 71 `data-zt` targets in the book, 0 dead.

### Gate

All lines below are the reruns after the Term relink.

- `check.mjs bedford2026learning`: top line 1 标题页与摘要 (2), 2 I. 引言 (19), 3 II. 概念框架 (41), 4 III. 相关性与度量 (160), 5 IV. 后果 (101), 6 V. 资本市场学习 (148), 7 VI. 摩擦 (111), 8 VII. 结论 (3), 9 附录 A (11), 10 附录 B (9), 11 附录 C (35), 12 表 1–10 (1), 13 图 1–10 (1), 14 Term list (160), 15 Cited list (69). Its only line: `868 Cards before, 872 now` (the four new summaries); no list fault, no unit change.
- Source-line walk (every Card with a source line, model parts excluded): every Card stands under its own top division, and each second-level branch holds only its own subsection's labels (3\|a §II.A; 3\|b §II.B, §3, §4; 4\|a–d §III.A–D; 5\|a §IV; 5\|b–d §IV.A–C; 6\|b §V; 6\|c–d §V.A–B; 7\|a §VI; 7\|b–c §VI.A–B; 8 §VII; 9 附录 A; 10 附录 B; 11\|a–f 附录 C.A–F). No crosser left.
- `list_broken_links` (agent): `{"broken":[]}`.
- `check-model.mjs` on the 12 new heads: `self_sufficient true`, `loose 0` on all (3\|b\|a members 16; 4\|a\|a 22; 4\|b\|a 12; 4\|c\|a 77; 5\|b\|b 33; 5\|c\|a 35; 5\|d\|a 32; 6\|a 62; 6\|c\|b 79; 7\|b\|a 39; 7\|c\|a 35; 7\|c\|b 33); the relation counts of the three formal models are those of stage 5.
- `verify.mjs --model` (light and dark): `3|b|a` `5 chart(s) in 17 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`; `4|b|a` `2 chart(s) in 13 card(s); …0…`; `6|a` `14 chart(s) in 63 card(s); …0…`; `apply.mjs` again `same` on all 18 Cards.
- Term relink: `{"rounds":1,"links_added":30,"applied":true,"cards_refused":0,"cards_changed":6,"ok":true,"links_repaired":0}`; `-PlanOnly` afterwards: `{"rounds":0,"links_added":0,"cards_changed":0,…,"applied":false}`. No relink bug.

## Result

Final top line: 标题页与摘要 · I. 引言 · II. 概念框架 · III. 管理者—企业匹配的相关性与度量 · IV. 管理者—企业匹配的后果 · V. 资本市场学习 · VI. 任命环节与状态转移中的摩擦 · VII. 结论 · 附录 A · 附录 B · 附录 C · 表 1 至表 10 · 图 1 至图 10 · 术语表 · 参考文献. 872 Cards (183 Idea, 63 Finding, 397 Model and part, 160 Term, 69 Cited).

Models (final addresses; old address in brackets):

- `bedford2026learning/3|b|a` [3\|a], formal: the conceptual framework of §II.B (binary latent match state, board hiring under frictions, two channels, learning after appointment), 16 members, illustrated (5 charts); II.B.
- `bedford2026learning/4|a|a` [5\|a], empirical: mixed-effects variance decomposition, match variance about ten times the manager variance (§III.A); lends equation (2) `4|a|a|a|2`.
- `bedford2026learning/4|b|a` [6\|a], formal: the directional-distance-function measure of match quality (§III.B), 12 members, imports `4|a|a|a|2` `单卡`, illustrated (2 charts).
- `bedford2026learning/4|c|a` [7\|a], empirical: validation of the score against the traditional match effects, with the Appendix A correlations (§III.C).
- `bedford2026learning/5|b|b` [8\|a], `5|c|a` [9\|a], `5|d|a` [9\|b], empirical: forced turnover, annual returns, announcement returns (§IV.A–C).
- `bedford2026learning/6|a` [10\|a], formal: the Bayesian learning model of §V and Appendix C, 62 members, illustrated (14 charts); first Side branch of the §V summary.
- `bedford2026learning/6|c|b` [11\|b], empirical: volatility on tenure deciles and initial match quality, split by within-tenure variance (§V.A–B).
- `bedford2026learning/7|b|a` [13\|a], `7|c|a` [13\|c], `7|c|b` [13\|d], empirical: frictions and initial match quality; frictions and transitions; industry shocks and transitions (§VI.A–B).

All seven stages passed their gates. Open items across the sequence (addresses as they now stand):

1. 30 identical-title Finding Card pairs remain (for example `4|a|b` = `4|a|d`, `4|c|b` = `4|c|f`, `7|c|c` = `7|c|i`, `9|c` = `9|e`); only the three pairs under the introduction were merged before stage 1. A `delete into` pass on the pairs whose body and statement fingerprint agree would remove 30 Cards; not done here (the organize moves and leads, it does not merge).
2. `6|a`'s 第 5 章 (`6|a|a|5`, §VI.B) states the Table 9 panel A frequencies and the Table 10 rates that `7|c|a` and `7|c|b` also state; no import was written (stage 4).
3. `4|b|a` imports `4|a|a|a|2` `单卡` by decision, so every score user tolerates it through `4|b|a` (stage 4).
4. In `6|a`, (C2) stands in three setting cards and (C4)–(C6) in two; two restatements stay inside proof chains ((C15) and the App. C.C proposition) (stages 2–4).
5. Result and derivation cards of the formal models keep their digest bodies; the old chapter cards keep their 问题/概要/关联 text under the new sections; copy advice left as judged comparisons between 3\|b\|a and 6\|a chapters (stages 3–4).
6. The 10 zt-check faults (R3/R13) in existing setting text of the three formal models and the import line of `4|b|a`; several figures draw 示意 shapes the source does not fix (stage 5).
7. No cross-book evidence pair for the nine empirical models: survey again after a zt-term-sync pass on the economics shelf; the judged record holds the 2,147 within-book pairs (stage 6). Survey noise: estimates without a Term of their own regressor pair with every estimate naming 匹配质量 (stage 6).
8. Stage 7: B.1 and B.2 are indexed by the framework Model Card; VI.A, VI.B, III.D, IV.B and C.C open on their first reading note; the Figure C1 panels stay under C.F; the illustrate modules keep their file names (`bedford2026learning-3_a.mjs`, `-6_a.mjs`, `-10_a.mjs`) with new heads. Nothing committed (stage brief).

## After Stage 7: duplicate Finding Cards (main session)

Date 2026-11-21, live box. Backup: `D:\zt\backups\zt-before-bedford2026learning-finding-dedup.sqlite3` (`VACUUM INTO`, integrity ok). Stage 7 reported 30 pairs of Finding Cards with identical titles. A script compared every Finding Card's `statement_fingerprint`: all 30 pairs were exact duplicates (same statement, no inbound Link on either copy), 63 Finding Cards for 33 statements, which is the 33 the first Findings pass created (`mid_applied_created 33`). The Findings retry of the digest wrote the already-applied groups a second time (a workflow fault; recorded in `docs/agents/workflow-principles.md`). Each later copy was deleted `into` the earlier one, from the highest address down so that no pending address shifted (30 deletes, 0 errors). The re-pointed Links left 19 "另见" lines that named one Card twice; a script kept the first of each and rewrote those 19 Cards (read back the same). Two **关联** prose lines and the tables lead that name a Card twice on purpose were left.

Gate: 33 Finding Cards, 0 duplicate fingerprints; `list_broken_links` (agent) empty; Term relink 0 changes; `check-model.mjs` `self_sufficient true`, `loose 0` on all 12 heads (3|b|a, 4|a|a, 4|b|a, 4|c|a, 5|b|b, 5|c|a, 5|d|a, 6|a, 6|c|b, 7|b|a, 7|c|a, 7|c|b). The Literature tree went from 872 to 842 Cards.
