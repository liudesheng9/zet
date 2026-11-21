# furceri2026divide: post-digest model passes

Furceri et al. (2026), *Divide and Indebt: How Geoeconomic Fragmentation Affects Public Debt and Macro-Fiscal Stability* (IMF WP/26/239), IMEN1 queue paper 9. The digest run report is `complete: true` (Bridge rebuilt 2026-11-14, 48/48 verified). Post-digest sequence (`zt-post-digest`) on the live box at the owner's request (2026-11-21, "Post-digest Furceri"). The papers 6–10 record ([imen1-postdigest-6-10]) left Furceri and Das unstarted on 2026-11-15; this pass starts Furceri.

## Stage 1: fold

Date 2026-11-21, live box. Backup: `D:\zt\backups\zt-before-furceri2026divide-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 9 Model Cards (8 empirical, 1 formal), 9 candidate groups (no rests-on Link joined any two; `read` pairs only: 6|a→4|a, 6|a→7|a, 7|a→4|a/8|a/9|a, 8|a→4|a/7|a/9|a, 9|a→4|a, 10|a→6|a).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 6\|a | 结果（五种识别方案、稳健性、传导渠道） | fragmentation shock → public debt | 6\|a | empirical; holds the estimand (effect of a one-s.d. GFI structural shock on debt/GDP at horizons 0–4 in the 90-country panel), the data tree and the specification ladder; the largest card (64 parts), kept as head |
| 1\|c | 摘要 | same | 6\|a | the abstract's verbal version of the whole analysis: same estimand, same panel |
| 3\|a | 引言 | same | 6\|a | the introduction's verbal version of the whole analysis: same estimand, same panel |
| 4\|a | 数据与样本 | same | 6\|a | the data and sample (GFI, macro-fiscal variables, 90 countries 1990–2022) the model uses |
| 5\|a | 实证策略 | same | 6\|a | the first stage of the same estimator: the panel SVAR (recursive and external narrative instrument) that extracts the GFI innovation the local projections use |
| 7\|a | 异质性分析：策略 | same | 6\|a | heterogeneity: the smooth-transition interaction of the same shock in the same local projections |
| 8\|a | 异质性结果：收入与贸易敞口 | same | 6\|a | heterogeneity: income group and trade exposure, same shock, same panel |
| 9\|a | 集团与结构特征 | same | 6\|a | heterogeneity: geopolitical bloc, institutions, financial openness, same shock, same panel |
| 10\|a | 在险债务 | debt-at-risk (location–scale quantile regression) | 10\|a | own model: a different estimand (the conditional quantiles of future debt, location and scale effects, the 95th percentile); its location effect equals 6\|a's mean response, which its 对应 Link already states |

The book holds **2 models**: `6|a` (empirical) and `10|a` (level formal: the location–scale quantile framework with its result). The evidence settled every row by the skill's rules (heterogeneity, the identification first stage, the data and the verbal versions belong to the estimand's model; a different estimand is a model of its own), so the fold ran without a question to the owner.

### Fold (executed 2026-11-21, live box)

Method: the per-card fold script of the IMEN1 passes (`fold.mjs` with batch retry, driven by `drive.mjs`, this session's scratchpad), one Card per run: strip **结构**/**引入**, mask every Link to a Model Card or part, park the parts and the Direct idea chains beside the section anchor, retype (facet, emphasis core, level cleared), move the head `alone` into 6\|a's main line, return the parts, restore the body with the rename map. 5\|a, 4\|a, 3\|a and 1\|c went in that order `after 6|a|a`, so they stand in book order at the top of the main line; 7\|a, 8\|a and 9\|a went to its end. The Direct idea chains 3\|a\|1–10, 7\|a\|1–5, 8\|a\|1–8 and 9\|a\|1–7 moved to their section anchors as Side branches. No folded head had an **引入** line. All 7 runs returned ok.

| Former head | Now | Facet |
|---|---|---|
| 1\|c | 6\|a\|a\|1 | 第 1 章：文字版本：摘要的整项分析 |
| 3\|a | 6\|a\|a\|2 | 第 2 章：文字版本：引言的整项分析 |
| 4\|a | 6\|a\|a\|3 | 数据章：GFI、宏观财政变量与 90 国面板 |
| 5\|a | 6\|a\|a\|4 | 识别章：两阶段面板 SVAR 提取 GFI 冲击 |
| 7\|a | 6\|a\|a\|11 | 异质性章：平滑转移局部投影框架 |
| 8\|a | 6\|a\|a\|12 | 异质性章：收入组与贸易敞口 |
| 9\|a | 6\|a\|a\|13 | 异质性章：地缘政治集团与结构特征 |

The facets are placeholders for the restructure, which rebuilds the spine and its chapter names.

### Gate

- `model-survey.mjs`: 2 Model Cards, `6|a` (313 parts) and `10|a` (7 parts), exactly the decided heads.
- `check-model.mjs 6|a`: every former head is a part; 6 `forward` faults (the design 6\|a\|a\|1 and chapter 13 name their own later parts) and 7 `unanchored` chapter cards (the folded heads), loose 3: left to the settings and restructure passes, as in the earlier books. `check-model.mjs 10|a`: self-sufficient, loose 0.
- Tree 636 Cards before and after; `list_broken_links` empty.
- Term relink: 2 Cards, 4 Links.

## Stage 2: settings (strict)

Date 2026-11-21, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-furceri2026divide-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The moves and deletes of 6\|a ran as seven `apply_batch` tree batches, one per folded head (11, 12, 13, 1, 2, 4, 3; 11–28 items each), and one reorder batch; each batch was previewed, its `deleted` set compared with the intended set, and committed under the plan hash (`links_that_will_break` empty in all). One preview of the head-3 batch was refused before commit: deleting a main-line or Direct-chain card deletes every later card of its chain, so the chain tail was deleted first and the former data chapter was parked as a Side of the Model Card before its delete. Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas of the old and new bodies; every written card was read back and compared with the intended text (0 differences). No display formula and no equation number changed in a card that stays. No `card_locked`, `scope_refused` or `read_view_changed` error occurred. The Literature tree went from 636 Cards to 543: 95 Cards were deleted `into` the cards they restate, 2 were created (the quantile specification of 6\|a and the notation card of 10\|a).

### 6\|a (empirical)

After the fold each of the seven folded chapters carried its own build: a data card, a 变量表, a design with one to seven assumptions, a specification ladder, and its sub-chapters of estimates on the data card's Direct chain. All of it was dissolved into one data tree, one design and one specification ladder. Addresses below are those before the pass unless marked "now".

- **Deleted `into` the card each restates** (Links re-pointed): the seven data cards into the data card `6|a|a`; the seven 变量表 into `6|a|a|a`; the designs 1\|a\|1, 2\|a\|1, 4\|a\|1, 11\|a\|1, 12\|a\|1, 13\|a\|2 into the design (`5`, now `6|a|a|1`); the identification assumptions of chapters 1, 2 and 4 into 假设 1–6 (GFI ordered first, common slopes, instrument relevance and exogeneity, no direct fiscal effect of events, common global shocks absorbed; the abstract's 假设 4 "fiscal stress does not feed back" into 假设 1; the lag length $p=2$, 4\|a\|1\|b, into the baseline specification); the heterogeneity assumptions of chapters 12 and 13 into 假设 7–11 (common shock, year fixed effects, lagged state variable, smooth transition, steepness $\theta$); the restating specifications of chapters 1, 2 and 4 (baseline SVAR, LP, GFI last, global block first, IV-SVAR, corrected and placebo instrument, crisis controls, sign restrictions and mean group, without US and China, sub-components, macro-fiscal outcomes) into the ladder cards they restate; the three copies of (4) of chapters 1, 2, 12 and 13 into the tagged (4) of chapter 11; the income and exposure specifications of chapter 12 into those of chapter 11; the bloc specification of chapter 13 (IPD, median-centred) into the alignment specification; the measures of the instrument (1\|a\|c, 4\|a\|b), of GFI (3\|a\|c), of income (12\|a\|b), IDGDI (12\|a\|c) and IPD (13\|a\|b) into the kept measures; the samples 3\|a\|b and the descriptive estimates 3\|a\|3\|a–c into the sample card; the whole former data chapter 3 (head, design, the two descriptive specifications, chapters 0 and 1 with their estimates) into the data card and the two descriptive cards; 12\|a\|d into the sample-mean description. The quantile specifications of chapters 1 and 2 (1\|a\|2\|a\|b, 2\|a\|2\|a\|b) went into the setting card of 10\|a, the model the fold gave that estimand.
- **Moved, because no card held them:** the state-dependent ladder of chapter 11 (base (4) and the income, exposure, alignment and structural variants) under the linear LP as `6|a|a|2|a|a`; the channel and external-debt specifications of chapter 12 and the bloc channel specification of chapter 13 under their dimension; the WGI and KAOPEN specifications of chapter 13 under the structural variant; the joint five-function and the alternative-form specifications under the base (4); the heterogeneity assumptions 11\|a\|1\|a–e, 12\|a\|1\|f, 13\|a\|2\|d–f to the design as 假设 7–15; the measures of the state variables (11\|a\|b–e, 13\|a\|c–d), of fiscal and financial variables (3\|a\|d), the sample (1\|a\|b) and the income-group sample (12\|a\|e) into the data tree; Table A1 under the sample-mean description; the two limits cards of chapter 3 to the Model Card (`6|a|c`, `6|a|d`); the hypothesis card 13\|a\|1 to chapter 13 (now `6|a|a|12|a`); the sub-chapters of every kept folded chapter up into its first Side slot, ahead of its limits cards.
- **Main line**: data, design, baseline, then the folded chapters 1, 2 and 4 (now `|3`–`|5`), chapters 1–4 (`|6`–`|9`) and the three heterogeneity chapters (`|10`–`|12`). The former data chapter 3 no longer exists.

```
6|a|a    数据：1990—2022 年 90 国非平衡面板                       原始设定
├ a  变量表 (one table, 38 rows; one row names every written form of a letter)
├ b  样本：基准估计样本与国家名单 → b|a 样本：收入分组
├ c  测量：GFI;  d 测量：财政与金融变量;  e 测量：叙事工具
├ f  测量：收入水平 m^GDP;  g 测量：贸易敞口 m^IDGDI;  h 测量：地缘政治结盟 m^IPD (with the B.3 median centring)
├ i  测量：结构性特征 → i|a WGI, i|b KAOPEN
├ j  描述：碎片化与平均债务同向变动 (with the 补写 pairing of Figure 1)
└ k  描述：基准样本的债务均值 (with the 补写 sample moments) → k|a 表 A1
6|a|a|1  设计 (identification and heterogeneity; 13 threats)
├ a 背景；b–g 假设 1–6 (shock identification)
└ h–p 假设 7–15 (common shock, year FE, lagged state, smooth transition, θ, IPD distance, functional form, correlated states, differential not absolute)
6|a|a|2  规格：基准递归 SVAR (1)(2)
├ a 线性局部投影 (3)
│   ├ a 状态依赖局部投影 (4): a 收入 → a|a 渠道; b 贸易敞口 → b|a 外债, b|b 渠道; c 地缘政治结盟 → c|a 集团渠道;
│   │     d 结构性特征 → d|a WGI, d|b KAOPEN; e 五个转移函数联合; f 替代的状态依赖形式
│   └ b 位置—尺度分位数回归 (5) (new)
├ b GFI 排最后; c 全球区块居首; d IV-SVAR → d|a 置换安慰剂, d|b 剔除四起事件
└ e 符号约束; f 均值组; g 剔除美国与中国; h 危机控制; i 四个子成分; j 宏观财政模块 → j|a 收支子成分; k 其他冲击
```

- **Data tree text.** Every sample, measure and description now has a one-line summary, **起点** and **内容** (the check's `start` faults). The data card lists the state variables and the heterogeneity outcomes and carries the two cross-book Links of the deleted data chapter (velic2025inflation/4\|a, rotunno2026industrial/2\|b\|19). The IPD measure took the B.3 median centring and the Western-bloc reading; the income and IDGDI measures took the **直觉** of the chapter-12 copies.
- **Design and assumptions.** The design states the heterogeneity comparison (the cross-regime difference $b_{0}^{h,s}$), seven heterogeneity threats and links all fifteen assumptions. The nine moved assumptions were renumbered 7–15 (titles, facets, the opening `假设 N：`).
- **Ladder text.** The state-dependent base got its facet `规格：状态依赖局部投影 (4)`, a `依赖` line to the linear LP and **相对基准的改动** (year fixed effects and the smooth-transition interaction added to (3)). The WGI and KAOPEN variants now rest on the structural variant. The new quantile specification `6|a|a|2|a|b` carries the source's (5) byte for byte from the deleted chapter-1 copy; the seven estimates of the debt and growth distribution in the verbal chapters (`6|a|a|3`, `3|a|4|a`–`d`, `4|a|3|a`–`b`) now rest on it instead of on 10\|a's setting card.
- **补写 equations (owner rule: every stage of a two-stage system).** Each of the 14 specifications that use the SVAR shock in a second stage (the linear LP, the state-dependent base, its four dimension variants, the three channel variants, external debt, WGI, KAOPEN, the joint and the alternative-form specifications) now also writes the first stage, under the line `第一阶段（原书 §II.B.1 的式 (2)，本卡照录；$\mathrm{shock}_{t}=e^{GFI}_{t}$ 一式按 §II.B 与 §II.B.2 的正文补写）`:
  $$\boldsymbol{A}_{0}\boldsymbol{Y}_{i,t}=\boldsymbol{\gamma}_{i}+\sum_{j=1}^{p}\boldsymbol{A}_{j}\boldsymbol{Y}_{i,t-j}+\boldsymbol{e}_{i,t},\qquad \mathrm{shock}_{t}=e^{GFI}_{t}$$
  The quantile specification writes the same first stage above (5) (15 new 补写 equations). Carried byte for byte from deleted digest copies (each with its source paragraph named): the IV-SVAR's own §II.B.1 form, the instrument as the first VAR variable and the first-stage analogue $u^{GFI}_{i,t} = \rho \, u^{Z}_{t} + v_{i,t}$, $W_{i,t} = \mu_i + \sum_{j=1}^{p} \varphi_j W_{i,t-j} + u_{i,t}$, into `2|d`, with its placebo ($\tilde Z$, $W^{pl}$) and corrected-instrument ($Z^{*}$, $W^{*}$) forms into `2|d|a` and `2|d|b`; the Figure 1 pairing $\bar d_t$ into `j` and the Table A1 sample moments into `k`. The other deleted copies wrote the kept equations in another notation (plain against bold letters, $A_0Y^{L}$ for the GFI-last ordering, $\kappa_1 GFC_t+\kappa_2 COVID_t+\lambda GPR_{i,t}$ for $\boldsymbol{G}\boldsymbol{x}_{i,t}$); they were not carried, so one notation stays.
- The 变量表 was rewritten as one table of 38 rows; bold and plain forms of one letter share a row (`$\boldsymbol{Y}_{i,t}$、$Y_{i,t-j}$`), and the letters with several uses ($\tau$: year effects and quantile operator; $\pi$: IV coefficient and scale coefficient; $d$, $v$, $N$) name all of them. The **结构** section of the Model Card was rebuilt from the build order (all parts except estimates, tests, figures, tables and conversions; 99 lines).

### 10\|a (formal)

```
10|a|a   设定：面板上的条件分位数写成位置成分加尺度成分 (5)          原始设定
├ a  符号表与读法 (new; blocks: 面板与协变量, 分位数与分布, 位置与尺度系数, 术语)
└ b  假设：线性位置—尺度形式、扰动中位数为零、分位数分别估计 (from 10|a|a|1, main line)   ← 10|a|a
```

- The assumption left the main line, so the derivations and the result renumbered: 推导：位置成分 is now `10|a|a|1`, 推导：总效应 `|2`, 结果 `|3` (with its example `3|a` and growth variant `3|b`).
- The setting card is in the format **起点** (原始设定) / **设定** (nine items; $\tau_q$ definition and (5) byte for byte) / **直觉** / **说明**; its symbol and term tables went to the notation card (21 rows, 2 terms), which also covers the indices and $F_Y$, $inv$, $\mathbf{x}$. The assumption card is in the format **起点** / **假设** / **直觉** / **说明**. The derivation `10|a|a|1` cited the assumption as "假设卡的第二条" (`stated`); it now links `[[10|a|a|b]]`. The **结构** section names the notation card.

### Gate

- `check-settings.mjs furceri2026divide/6|a`: `settings: the data tree and the specification ladder are in place` (no fault, no advice line).
- `check-settings.mjs furceri2026divide/10|a`: `settings: all in the setting part, as a dependency tree` (no fault, no advice line; every `reads:` symbol is defined in the setting card).
- `check-model.mjs 6|a`: 219 members (13 main, 203 side, 3 off), loose 3; 4 `forward` faults (the verbal chapter 1 head `6|a|a|3` and chapter 13 head `6|a|a|12` name their own later parts) and 4 `unanchored` folded chapters (`6|a|a|4`, `|5`, `|10`, `|11`), so `self_sufficient` stays false. Left to the restructure, as after stage 1.
- `check-model.mjs 10|a`: 8 members, loose 0, `self_sufficient` true.
- `list_broken_links` (agent): empty, before and after the relink. Tree 543 Cards (636 − 95 deleted + 2 created).
- Term relink: `{"book":"furceri2026divide","applied":true,"rounds":1,"links_added":38,"cards_changed":11,"cards_refused":0,"ok":true,"links_repaired":0}`. Both settings checks and `list_broken_links` were rerun after it and stay clean.

### Open

1. 6\|a: the folded chapters keep their placeholder facets and stand after the ladder; the verbal chapters 1 and 2 (`6|a|a|3`, `|4`) and the identification chapter (`|5`) restate estimates of chapters 1–4 (`|6`–`|9`), and four of them are unanchored. The restructure builds the evidence spine and decides the merges.
2. 6\|a: `6|a|a|2|a|b` restates 10\|a's (5) in 6\|a's notation for the seven distribution estimates of the verbal chapters; stage 4 decides whether it becomes an **引入** of 10\|a, or whether those estimates move to 10\|a.
3. 6\|a: `2|d` keeps the digest's short form $\mathrm{shock}_{t}=\pi Z_{t}+\nu_{t}$ with a second stage on $\widehat{\mathrm{shock}}$ beside the source's internal-instrument form; the short form does not match §II.B.1 (the IRF comes from the augmented VAR, not from an LP on a fitted shock). It was kept because no display formula may change; the owner may decide to drop it.
4. 6\|a: two notations stay side by side in the ladder (bold $\boldsymbol{c}$, $\boldsymbol{Y}$ in (3) and the chapter-11 copies; plain $c$, $Y$ in the chapter-12 and 13 variants); the 变量表 names both.
5. 6\|a: the assumption numbers of the heterogeneity analysis changed (7–15); estimate and test cards that name an assumption by number in prose (not by Link) still use the old chapter-local numbers. The restructure should check them.

## Stage 3: restructure

Date 2026-11-21, live box, under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; chapter cards written by hand, estimate and test cards redrafted by a script that rebuilds each card in its role format and carries the facts of the merged duplicates). Backup: `D:\zt\backups\zt-before-furceri2026divide-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). 6\|a's moves and merges ran as three `apply_batch` tree batches (A: 31 moves; B: 32 deletes `into`; C: 27 deletes `into` and 5 reorders), 10\|a's as two (one `insert_card`; two moves and one reorder); each batch was previewed, its `deleted` set compared with the intended set, and committed under the plan hash (`links_that_will_break` empty in all). Text ran as `edit_card` calls under `-Allow` (6\|a: 96 + 6; 10\|a: 8); every written card was read back and compared with the intended text (0 differences). Before each write a script compared the display formulas and the `\tag` numbers of the old and new bodies, and checked that no image and no Link outside the chapter pointer lists was lost. After the pass the two models hold 61 display formulas (39 distinct) and 6 `\tag` numbers, as before; none of the deleted cards held a display formula. No estimate number was changed; numbers that only a deleted duplicate stated were carried into the card that kept the estimate. No `card_locked`, `scope_refused` or `read_view_changed` error occurred. The Literature tree went from 543 to 485 Cards (59 deleted `into` the cards that hold their content, 1 created: the chapter card of 10\|a).

### 6\|a (empirical): the evidence spine

```
6|a|a     数据 → |1 设计 (假设 1–15) → |2 规格：基准递归 SVAR (ladder as stage 2 left it)
6|a|a|3   第 1 章：直觉（摘要与引言）            ← 规格                     a 局限：引言只给摘要
6|a|a|4   第 2 章：主要估计（§II.C.1）           ← 规格                     a 图 2; b 基准递归 → b|a 换算; c 局部投影; d GFI 居末 → d|a 检验;
                                                                         e 全球区块居首 → e|a 检验; f IV-SVAR; g 引言的 1–3 个百分点 → g|a 检验;
                                                                         h 幅度比较 → h|a 图 3; i 子成分 → i|a 图 4; j 外部有效性
6|a|a|5   第 3 章：识别检验（§II.B.1）           ← 2|d, 第 2 章              a 检验：第一阶段; b, c 局限
6|a|a|6   第 4 章：稳健性（§II.C.2）             ← 规格, 第 2 章             a 表 1; b 符号约束; c 均值组 → c|a 检验; d 剔除美中 → d|a 检验;
                                                                         e 安慰剂 → e|a 检验; f 剔除四起事件 → f|a 检验, f|b 表 s13-2;
                                                                         g 危机控制 → g|a 检验, g|b 表 s13-3
6|a|a|7   第 5 章：传导渠道（§II.C.3）           ← 2|j, 第 2 章              a 图 8; b–g 产出、物价、总赤字、支出与收入、税种与支出类别、利差
6|a|a|8   第 6 章：状态依赖框架（§III.A）         ← 2|a|a, 第 2 章, 数据      a θ 的敏感性 → a|a 检验; b 局限
6|a|a|9   第 7 章：收入水平（§III.B.1）          ← 2|a|a|a, 第 6 章          a 图 10; b, c 估计; d 表 s13-4; e 发达与西方（引言）; f 图 11; g–i 估计; j 局限
6|a|a|10  第 8 章：贸易敞口（§III.B.2）          ← 2|a|a|b, 第 7 章          a 图 12; b, c 估计; d 图 13; e, f 估计
6|a|a|11  第 9 章：地缘政治集团（§III.B.3）       ← 2|a|a|c, 第 7 章          a 图 14; b 估计; c 表 s13-5; d 图 15; e–g 估计; h 局限
6|a|a|12  第 10 章：制度与金融开放（§III.B.4）    ← 2|a|a|d (d|a, d|b), 第 9 章  a 假说; b 图 16; c, d 估计
6|a|a|13  第 11 章：状态依赖的稳健性（§III.B.5）  ← 2|a|a, 第 10 章          a 图 17; b 联合 → b|a 检验; c 图 18; d 替代形式 → d|a 检验
6|a|a|14  第 12 章：债务分布与在险债务（§IV）     ← 2|a|b, 第 2 章            a 位置; b 尺度; c 在险债务 → c|a 换算; d 在险增长
6|a|b, c, d  局限 (Model Card side, unchanged place)
```

Network (`check-network.mjs`): 数据 → 设计 → 规格; under 规格 chapters 1, 2, 3, 4, 9, 10, 11; under 第 2 章 chapters 5, 6 and 12; 6 → 7 → 8. **分部** (new in the Model Card): 数据与识别 from `6|a|a`; 平均效应 from 第 1 章; 国家之间的差异 from 第 6 章; 债务分布的尾部 from 第 12 章.

- **Stage-2 open item 1 (the verbal and identification chapters) resolved.** The abstract chapter (former `|3`) became 第 1 章：直觉. Its **直觉** states the paper's question in three layers, the data and the two identifications; its **本章估计** are pointer lines from each statement of the abstract and the introduction to the estimate card that gives it (no premise; the chapter rests on the baseline specification only). The introduction chapter (former `|4`) was deleted `into` it after its limits card moved under it. The identification chapter (former `|5`) was deleted `into` the identification-test chapter (now 第 3 章), whose **直觉** took its two-stage description and whose **关联** took its four Links (book §II.B.1, Literature card 6, riboni2026presidential/4\|f, goldsmithpinkham2020bartik/2\|b\|a).
- **Duplicates merged** (delete `into`, leaf first, Links re-pointed): from the abstract chapter 29 cards (its five inner chapters into 第 2, 3, 4, 5 章 and 第 12 章's head; 24 estimates, tests and figures into the cards of chapters 2, 4, 5, 7–9, 11 that state the same estimate), from the introduction chapter 10, from the identification chapter 16, and the three inner chapter cards `10|a`, `11|a`, `12|b` into their chapter heads. Facts only a deleted copy stated were carried into the kept card: the LP readings of Figure 2 (2.7, 1.3, 1.0), the Figure 3 readings (−0.9, +1.9, 2.3/3.2/2.8), the year-4 value and 60% interval of the corrected instrument (2.8; 3.3–4.1), the year-0/1 values of the crisis controls (1.95, 1.60), the four removed events, the crisis years, the Figure 5 middle-panel range (2.8–2.9) and right-panel band (−5 to 5), the introduction's output (1.5%–1.6%), spending (0.4 pp in the medium term) and spread wording, the Figure 12 year-4 reading, the Figure 16 starting values (2.3, 1.3), and the reading lines of five estimates. Links of deleted cards carried: `对应 [[furceri2026divide/1|e]]`, `/3|b`, `/3|c`, `/3|d` into the estimates of chapters 4 and 5; 假设 1 into the test of the US–China exclusion; 假设 10 into the functional-form test; the mean-group and sign-restriction specifications into the slope test. Images of deleted cards that the kept card lacked were appended to it.
- **Moved, because no card held them:** the introduction's 1–3 pp estimate with its test (`4|g`), the two ordering tests (`4|d|a`, `4|e|a`), the slope-homogeneity test (`6|c|a`), Tables s13-2 to s13-5, the introduction's advanced-and-Western estimate (`9|e`), the θ estimate, the hypothesis card (to 第 10 章, the chapter whose dimension it predicts) and the limits cards. The distribution chapter (former abstract inner chapter 5) went to the end of the main line as 第 12 章; the exposure, structure and state-robustness inner chapters went to the main line as 第 8, 10 and 11 章.
- **Step 3 redraft done.** Every estimate card now reads: one sentence; **估计** (估计量 with its specification Link and the table, figure or text it comes from; 值; 标准误 or 不确定性; 显著性; 样本量; the digest's separate 规格 and 出处 items folded into 估计量); **量级**; **为什么**; **依据** (`依赖` the specification, `识别见设计`, `检验见` its tests, rests-on Links to figure cards kept); **对应**; 来源 with the section. Every test card reads: one sentence; **检验什么** (`依赖` the assumption); **做法** (`依赖` the specification); **结果**; **结论**; `印证` the estimate; 来源. The lines that only said a card belongs to a chapter (`它属于第 k 章`, `本估计是该章的一张估计卡`) were dropped; figures, tables and limits filed directly under a chapter say `本卡属于第 k 章 [[…]]`. Every chapter card is **直觉** (lead of 20+ characters), **本章估计** (pointer lines), **承上启下** (`依赖` the specification and the chapter it extends), **关联** (cross-book and book Links of the former heads), 来源; none of the old 问题/概要/估计对象/设计/主要估计/结构 text stays. The Model Card's **结构** keeps its setting lines and lists the twelve chapters with one level under each; its 关联 and source lines now cover §摘要–§IV.
- **Stage-2 open item 5 resolved.** The prose mentions of assumptions by old chapter-local numbers were in the folded heads' **结构** lists (rewritten) and in the θ test (`假设 5` → `假设 11`, beside its Link). A scan finds no assumption named by number in prose without its Link, and every number matches the assumption card's facet.
- **Setting part, text only:** a **直觉** lead was prepended to the data, design and baseline specification cards (the network's `lead` check); in the 变量表 the row of $F(m)$ said `基于中位数的虚拟变量`, which zt read as a rests-on cue to the specification it names (a `forward` fault from the 变量表); it now says `按中位数构造的虚拟变量`. The two loose Model Card limits (`6|a|c`, `6|a|d`) got one plain reference line each, to the descriptions `j` and `k`/表 A1.

### 10\|a (formal): the intuition spine

```
10|a|a        设定 (a 符号表与读法, b 假设; as stage 2 left them)
10|a|a|1      第 1 章：位置与尺度（§IV） (new)        ← 10|a|a, 10|a|a|b
  a  结果：上尾响应大于下尾 (former 10|a|a|3)
     a  推导：位置成分 → a|1 推导：总效应   (proof chain, former main line |1, |2)
     b  数值例子;  c  变体：增长分布
```

The derivations left the main line: a new chapter card holds the result as its Side branch, and the two-step proof chain hangs under the result. The result card is in the result format (one sentence, **结果** with the original text and figure, **为什么**, **证明** with its two `依赖` lines). The derivations got a `目的` line, `推导链第 k 步，共 2 步`, the mark `（论文略去，此处补出）` for the skipped steps, and pointers to the next step and to the result. The notation card got a **怎样读卡片上的记号** section. **结构** rebuilt. One chapter, so no **分部**.

### Gate

- `check-model.mjs 6|a`: `members 160 {"main":15,"side":142,"off":3}`, `relations {"rests-on":152,"refers":257,"corresponds":3,"illustrates":12}; loose 0; self_sufficient true`; no closure, forward or unanchored fault (was 4 forward, 4 unanchored, loose 3).
- `check-model.mjs 10|a`: `members 9 {"main":2,"side":7}`, loose 0, `self_sufficient` true.
- `check-network.mjs`: `network: clean` for both (6\|a: 16 blocks, 7 layers; 10\|a: 2 blocks, 2 layers).
- `check-premises.mjs 10|a`: `premises: none used before their card`. `6|a`: 9 hits, all in the setting part and all pointers: Table A1 (`6|a|a|k|a`) labels its five columns `(1)`–`(5)`, which the check reads as equation numbers; 假设 2, 5 and 6 (`1|c`, `1|f`, `1|g`) say how they enter (1) and (2). The data → design → specification order of the evidence spine is fixed, as in riboni2026presidential.
- `check-settings.mjs`: 6\|a `settings: the data tree and the specification ladder are in place`; 10\|a `settings: all in the setting part, as a dependency tree`, with one advice line (the chapter card rests on the assumption card by its Link, which is the intended citation).
- `list_broken_links` (agent): `{"broken":[]}`. Literature tree 543 → 485 Cards.
- Term relink: `{"book":"furceri2026divide","applied":true,"rounds":1,"links_added":100,"cards_changed":30,"cards_refused":0,"ok":true,"links_repaired":0}`. All checks above and `list_broken_links` were rerun after it and stay as listed.

### Open

1. Stage-2 open item 2 stays for stage 4: `6|a|a|2|a|b` restates 10\|a's (5); the four distribution estimates of 第 12 章 (`6|a|a|14|a`–`d`) rest on it.
2. Stage-2 open items 3 and 4 (the IV short form in `2|d`; two notations in the ladder) stay.
3. Some estimate cards keep the digest's source pointers in the source-file form (`s10.md:7`, `s11.md:5`) inside 估计量 and 量级; the 来源 line now names the section.
4. 第 1 章 has no estimate card of its own (pointer lines only), as the intuition chapter of the spine.
5. Tree unlocked throughout; no `card_locked` met.
