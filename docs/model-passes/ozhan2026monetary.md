# ozhan2026monetary: post-digest model passes

Ozhan, Sander, Wende and Yang, *Monetary Policy Under Network-Level Bottlenecks* (IMF WP/26/244), Work 29957. The digest is complete in its ledger (`run_complete:true`); its run report lists one gap, a figure the source gives no label. The Bridge was rebuilt fresh on 2026-11-14. All passes ran on the live box (`D:\zt`), 2026-11-14.

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-ozhan2026monetary-fold.sqlite3` (integrity_check ok).

The survey found 22 Model Cards (5 concept, 15 formal, 2 empirical). Several rows needed judgement (10\|a, the Appendix A cards, 19\|a, 20\|a). The proposal was shown to the owner, who approved it on 2026-11-14 ("Yes, fold as proposed").

| Model Card | Level | Model | Surviving head | Reason |
|---|---|---|---|---|
| 5\|a | formal | stylized model (§2) | 5\|a | households and upstream production with a task-based capacity cap |
| 6\|a | formal | stylized model | 5\|a | §2.2.2: downstream production of the same two-sector economy |
| 7\|a | formal | stylized model | 5\|a | §2.3: market clearing and the feedback rule that close the same model |
| 8\|a | formal | quantitative model (§3) | 8\|a | two regions, 11 sectors, two household types |
| 3\|a | concept | quantitative model | 8\|a | the §1 verbal version of the quantitative model; first chapter |
| 9\|a | formal | quantitative model | 8\|a | §3.2.1: the firms' CES production and labour cap |
| 9\|b | formal | quantitative model | 8\|a | §3.2.2: Calvo pricing and dominant-currency pricing |
| 10\|a | formal | quantitative model | 8\|a | §3.3–3.5: Taylor rule, government and market clearing of the same model; its rest on the stylized feedback rule is a borrowed result, kept as an **引入** line |
| 11\|a | formal | quantitative model | 8\|a | §3.6: extensions of the same setting (sticky wages, imperfect labour mobility, fixed capital) |
| 17\|a | concept | quantitative model | 8\|a | §5.2: feature-by-feature experiment on the same model (Hulten benchmark) |
| 18\|a | concept | quantitative model | 8\|a | §5.3: sectoral labour-cap experiment on the same model (Phillips curve) |
| 22\|b, 23\|a, 24\|a, 25\|a, 26\|a, 27\|a | formal | quantitative model | 8\|a | Appendix A: the full OLG form of the same model's households, aggregates, demand system, production, pricing and closure |
| 13\|a | empirical | fit to 2020–24 data (§4, §6.2) | 13\|a | its own data (48 observables, two regions, 11 sectors, 2020Q1–2023Q4) and its own shock extraction; it takes the quantitative model as a whole |
| 14\|a | empirical | fit to data | 13\|a | §4.2: the quasi-nonlinear shock extraction on the same data |
| 15\|a | concept | fit to data | 13\|a | §4.3: the counterfactual method that keeps the estimated constraint states |
| 20\|a | concept | fit to data | 13\|a | §6.2: the decomposition of the fitted supply-constraint contribution |
| 19\|a | formal | DC-index weighting experiment (§6.1) | 19\|a | own question (how to weight sectoral inflation) and own multi-sector setting |

The book holds **4 models**: `5|a` (formal), `8|a` (formal), `13|a` (empirical) and `19|a` (formal).

### Fold (executed 2026-11-14, live box)

- 5\|a: `5|a|a|9` 第 2 章：下游生产与迂回投入 (was 6\|a), `5|a|a|10` 第 3 章：市场出清与利率规则 (was 7\|a).
- 8\|a: `8|a|a|1` 第 1 章：文字版本：网络与劳动上限 (was 3\|a); `8|a|a|17` 企业生产 (9\|a); `|18` 企业定价 (9\|b); `|19` 政策、政府与市场出清 (10\|a); `|20` 扩展 (11\|a); `|21` Hulten 基准与模型特征 (17\|a); `|22` 劳动约束与菲利普斯曲线 (18\|a); `|23`–`|28` the six Appendix A chapters (22\|b, 23\|a–27\|a).
- 13\|a: `13|a|a|4` 准非线性冲击提取 (14\|a), `|5` 准非线性反事实模拟 (15\|a), `|6` 供给约束的贡献分解 (20\|a).
- Each folded card's setting or data parts are filed under it; its Direct idea notes moved to its section as Side branches; the idea Side cards of 15\|a stay in section 15.
- The dropped **引入** lines of 6\|a, 7\|a, 9\|a, 9\|b, 23\|a and part of 10\|a named parts of their own model after the fold. 10\|a's line to the stylized model's feedback rule (`5|a|a|10|a|1`) names another model; it was appended to the **引入** section of 8\|a, which removed the closure fault.

### Gate

- `model-survey.mjs`: exactly 4 Model Cards: 5\|a (30 parts), 8\|a (148), 13\|a (45), 19\|a (9). 8\|a builds on 5\|a by the imported rule; 5\|a has one `false share` to a part of 8\|a, left to the import stage.
- `check-model.mjs`: every former head is a part; 19\|a `self_sufficient` true. The remaining faults are the unanchored folded chapters (5\|a: 2; 8\|a: 13; 13\|a: 3) and loose parts (1, 2, 5), which the settings and restructure passes fix.
- `list_broken_links`: empty. `term-relink.ps1`: `{"links_added":3,"cards_changed":2,"ok":true}`.

### Open

- 8\|a's Appendix A chapters restate much of §3 (the appendix text says so: "书中此处与 7.1 逐字相同"); the settings and restructure passes merge restatements.
- 5\|a's false share to 8\|a and the stylized-rule import become proper **引入** lines in stage 4.
