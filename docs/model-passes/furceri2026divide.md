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
