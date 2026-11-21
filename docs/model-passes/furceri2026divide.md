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
