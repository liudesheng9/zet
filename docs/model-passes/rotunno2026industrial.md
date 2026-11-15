# rotunno2026industrial: post-digest model passes

Rotunno, Ruta and Verma, *Industrial Policy and Trade Tensions in Strategic Sectors* (IMF WP/26/155), Work 29955. The digest is settled with two gaps the owner accepted (skeleton/book@2 and the cleanup wait); see `D:\zt-digest\_queue\imen1-queue.md`. All passes ran on the live box (`D:\zt`), 2026-11-14.

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-rotunno2026industrial-fold.sqlite3` (integrity_check ok).

The survey found 5 Model Cards and 5 candidate groups. No rests-on Link joined any two of them. The evidence settles every row, so the fold ran at once.

| Model Card | Level | Model | Surviving head | Reason |
|---|---|---|---|---|
| 4\|a | formal | quantitative trade model | 4\|a | the §3 model: N countries, J sectors, external economies of scale, subsidies and tariffs as price wedges |
| 2\|a | concept | quantitative trade model | 4\|a | the §1 verbal version of 4\|a; the Card itself says it is only the conceptual entry to the same framework; it becomes the model's first chapter |
| 9\|a\|3 | formal | quantitative trade model | 4\|a | Appendix A.1: the equilibrium conditions and the hat-algebra form of the same model; same agents, technology and policy instruments; its five **引入** lines all named 4\|a's own parts and were dropped |
| 3\|a | empirical | subsidy measurement | 3\|a | §2: the amount–count relation estimated on NIPO and the back-cast subsidy rates; own data and estimand; the theory only takes its rates as input |
| 10\|a | empirical | subsidy measurement | 3\|a | Appendix A.2: the test of 3\|a's extrapolation (GTA prediction against NIPO observation, Table A.2), same data and estimand; it becomes the model's fifth chapter |

The book holds **2 models**: `4|a` (formal) and `3|a` (empirical).

### Fold (executed 2026-11-14, live box)

- 2\|a → `4|a|a|1`, facet "第 1 章：文字版本：补贴、规模经济与三类效应（§1）", its setting chain filed under it.
- 9\|a\|3 → `4|a|a|13`, facet "附录章：均衡条件与变化率形式（附录 A.1）", its setting and solution parts filed under it. The idea notes 9\|a\|4–6 stay in section 9.
- 10\|a → `3|a|a|7`, facet "第 5 章：预测检验（附录 A.2）", its data and limitation parts filed under it. Its 17 idea notes (the A.2 tables and figures) moved to section 10 as `10|e` and its chain.
- Incident: the first retype of 10\|a was refused because the facet was longer than 32 characters, after the parking had applied. The retype, the move and the body restore were completed by a recovery script; the masked body Links were restored to the new addresses.

### Gate

- `model-survey.mjs`: exactly 2 Model Cards, 3\|a (63 parts) and 4\|a (40 parts).
- `check-model.mjs`: every former head is a part. Remaining faults are the unanchored new chapters (`4|a|a|1`, `4|a|a|13`, `3|a|a|7`), which the restructure fixes.
- `list_broken_links`: empty. `term-relink.ps1`: 1 Link added, 1 Card changed.

### Open

- 3\|a and its new chapter 5 both carry the Table A.2 specifications and estimates (3\|a\|a\|4 and 3\|a\|a\|7). The settings and restructure passes reconcile the duplication.

## Stage 2: settings (strict)

Date 2026-11-14, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-rotunno2026industrial-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The moves, the deletes and the creation of the 4\|a notation card ran as two `apply_batch` tree batches, one per model (preview, then commit under the plan hash; `links_that_will_break` empty in both). Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the `\tag` numbers of the old and new bodies, and every written card was read back and compared with the intended text (0 differences). No display formula and no equation number changed in a card that stays; the deleted restatements carried their own copies of the Table A.2 equations, which are named below. No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 547 Cards to 530: 3\|a lost 17 Cards, 4\|a lost one restating assumption card and gained one notation card.

### 3\|a (empirical)

The folded chapter 5 (`3|a|a|7`, Appendix A.2) carried a second build of the model: its own data card, 变量表, design, four identifying assumptions, a five-step specification ladder and three sub-chapters of estimates and tests. It was dissolved into the model's own data tree, design and ladder in one batch of 25 items.

- Deleted `into` the card each restates (Links re-pointed): the A.2 变量表 into `3|a|a|a` (the main table already had every A.2 symbol in the ladder's notation); the A.2 design into the design `3|a|a|1`; 假设 1 (predicted and observed values measure the same object) into 假设 3 `3|a|a|1|c`; the A.2 specifications 基准, 部门固定效应, 国家固定效应 and 两类固定效应 into `3|a|a|2|c`, `2|c|a`, `2|c|b` and `2|c|c`; the A.2 cross-validation specification (no fixed effects) into `3|a|a|2|b`; the A.2 table card into `3|a|a|4|a`; the 0.594 and 0.407 estimates into `3|a|a|4|b` and `4|c`; the fixed-effects test into `4|c|a`; the 47.3 estimate into `3|a|a|5|a`; the functional-form test into `3|a|a|5|a|a`; the three A.2 sub-chapters into chapter 2 (`3|a|a|4`, two of them) and chapter 3 (`3|a|a|5`). The four deleted specifications wrote the Table A.2 regressions as $\Delta S_{ij}^{GTA}=\alpha+\beta\,\Delta S_{ij}^{NIPO}+\ldots+\varepsilon_{ij}$; the kept ladder writes the same regressions as $\kappa+\theta_3\,\Delta S_{ij}^{NIPO}+\ldots+u_{ij}$, so one notation stays. The A.2 cross-validation equation $\hat f=\arg\min_{f\in\mathcal F}\mathrm{MAD}^{\mathrm{CV}}(f)$ is the same selection as the kept 补写 equation of `3|a|a|2|b`.
- Moved, because no card held them: the A.2 data card to the data tree as `样本：表 A.2 的 2023 年单元` (`3|a|a|e`, **起点** now rests on the data card); its measure of the predicted change to `3|a|a|f`; the A.2 assumptions 2–4 to the design as 假设 5–7 (`3|a|a|1|e` relation not driven by country or sector differences, `1|f` linear form preferred, `1|g` 2023 reporting basis does not distort the observed values); the sector-FE and country-FE slopes (0.526, 0.543) to chapter 2 as `3|a|a|4|d`, `4|e` (their 估计量 now names $\theta_3$); the no-FE MAD estimate (about 20) to chapter 3 as `3|a|a|5|b`.
- Text merged into the kept cards: the design took the A.2 comparison sentence, 假设 5–7 in its list and two threats (the 2022 zero-subsidy basis, 1–2 log-point errors); 假设 3 took the A.2 reading of the same-object assumption; `3|a|a|4|a` took the four-column estimates table of Table A.2; `4|b` and `4|c` took **对应** lines to 假设 3 and 假设 5; the tests `4|c|a` and `5|a|a` now also cite 假设 5 and 假设 6; chapters 2 and 3 list their new estimates; the chapter-5 head points to the ladder and to chapter 2.
- The measures `3|a|a|b`, `c`, `d` and `f` now have a one-line summary, **起点** and **内容**. `3|a|a|2|b` now names $\mathcal{F}$ in prose (its equation writes it only in a subscript, so the 变量表 row read as unused).
- One specification ladder remains: `规格：基准` (1) with the variants 不含固定效应, 10 折交叉验证选模, 表 A.2 无固定效应 (with its three fixed-effects variants) and 回溯. Every specification already had its estimating equation (the variants as 补写 equations); none is an IV specification, so no equation was added.

### 4\|a (formal)

```
4|a|a   设定：N 国 J 部门、两层偏好与三种政策工具 (2)        原始设定
├ a  符号表与读法 (new; blocks: 国家与部门, 技术, 劳动供给、政府与贸易逆差, 推导中的辅助符号, 术语)
├ b  设定：概述（§1） (from 4|a|a|1|a, the §1 verbal setting)  原始设定
├ c  设定：部门单位成本指数 (3) (from 4|a|a|2)                ← 4|a|a
│    c|1 假设：Fréchet 生产率 (4) (from 4|a|a|3)              ← 4|a|a, c
├ d  设定：劳动供给、政府与逆差 (from 4|a|a|13|a, A.1; 4|a|a|4 merged in)  ← 4|a|a, c, c|1
│    d|1 假设：补贴先于关税征收 (from 13|a|1)                  ← d
│    d|2 设定：总收入 (A.3) 与最终消费价格指数 (A.4) (from 13|a|2, formerly facet 定义)  ← d, d|1
└ e  设定：校准（2015 年基年） (from 4|a|b, formerly facet 校准与求解)  ← 4|a|a, c, c|1
```

- The settings on the main line (4\|a\|a\|2–4) and in the folded chapters (1\|a, 13\|a with 13\|a\|1–2) left for the setting part, so the main line renumbered: the derivations 5–12 are now 4\|a\|a\|2–9 and the appendix chapter 13 is now `4|a|a|10`. Chapter 1's mechanism chain rose into its Side slot (`4|a|a|1|a`, `1|a|1`, `1|a|1|a`); chapter 10 opens on its derivation chain `10|a`–`10|a|13|a`, and the solution algorithm stays there as `10|b`.
- Deleted `into` `4|a|a|d`: the §3 assumption 4\|a\|a\|4 (trade balance fixed, government net spending returned lump sum), which the A.1 setting restates with symbols. Its fixed-at-base-year statement went into item 3 of `d`, and its remark that §3 shows only the gravity equation into **说明**. It held no display formula.
- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**. The two symbol tables and two term tables went into the notation card (43 rows, 7 terms), which covers every symbol of every setting and the auxiliary symbols of the derivations ($\pi_{in}^j$, $Exp_i^j$, $\bar r_{ni}^j$, $\Delta$, $q_i^j$, $Z_{in}^j$, $\nu_{in}^j$, $\lambda_i^j$, $\zeta$, $\xi_n^j$, $\kappa_n^{js}$). Each row names the card where a reader meets the symbol first, as the check reads it.
- Statements moved and pointers: `d` dropped the restated countries, sectors and varieties of the setting card; `d|1` dropped its restated exogenous deficit; the calibration `e` dropped the solution-algorithm sentences, which `10|b` states, and points to it in **说明**; it describes the subsidy-rate change in words, since the inline $\hat{\tilde s}_i^j$ was read as two symbols. The setting card names its block order in **说明**. The chapter-10 overview now opens its first part on `10|a`, with a pointer to `d`.
- Definitions added at first use: $U_i$ and $C_i^j(\omega)$ in the setting card; $z_i^j(\omega)$ (with a look-up to `c|1`) in `c`; $Y_i$ (look-up to `d|2`), $X_i^j$ and $X_{in}^j$ in `d`; $P_n$ and $\alpha_n^j$ in `d|2`.

The **结构** sections of the Model Cards 3\|a and 4\|a were rebuilt from the build order (setting part first, then the main line).

### Gate

- `check-settings.mjs rotunno2026industrial/3|a`: `settings: the data tree and the specification ladder are in place`.
- `check-settings.mjs rotunno2026industrial/4|a`: `settings: all in the setting part, as a dependency tree`.
- `check-model.mjs 3|a`: 46 members (8 main, 37 side, 1 off), loose 2; `unanchored` on chapter 5 (`3|a|a|7`), so `self_sufficient` stays false. Left to the restructure.
- `check-model.mjs 4|a`: 40 members (11 main, 29 side), loose 0; `unanchored` on chapter 1 (`4|a|a|1`) and the appendix chapter (`4|a|a|10`), as after stage 1. Left to the restructure.
- `list_broken_links` (agent): empty, before and after the relink.
- Term relink: `{"cards_changed":12,"book":"rotunno2026industrial","links_added":46,"cards_refused":0,"rounds":1,"links_repaired":0,"ok":true,"applied":true}`. Both settings checks and `list_broken_links` were rerun after it and stay clean.

### Open

1. 3\|a: chapter 5 (`3|a|a|7`, Appendix A.2) now holds only its head and the limits card `3|a|a|7|b` (Side slot `a` is free). The head restates the Table A.2 regression with its own display formula ($\alpha,\beta,\gamma_j,\delta_c$ notation), which is kept because it stands nowhere else byte for byte; chapter 2 (`3|a|a|4`) holds the same test's estimates. The restructure decides whether chapter 5 is merged into chapter 2.
2. 3\|a: chapter 2 lists its slopes in the order (1), (4), (2), (3) (`4|b`, `4|c`, `4|d`, `4|e`); a reorder is left to the restructure.
3. 4\|a: the notation card carries a row `$A$` for the appendix equation prefix, because the check reads the `A` of `(A.3)` inside the display formula as a symbol; the formula was not changed.
4. 4\|a: the §1 verbal setting `b` restates the calibration scope (ten countries plus ROW, 20 sectors) that `e` states; the check finds no repeat and it stays as the overview.
5. 4\|a: chapters 1 and 10 are unanchored; the derivations of §3 (`4|a|a|2`–`9`) stand on the main line between them. The restructure builds the spine.

## Stage 3: restructure

Date 2026-11-14, live box, under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; the chapter leads were written by hand). Backup: `D:\zt\backups\zt-before-rotunno2026industrial-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Moves ran as two `apply_batch` tree batches (4\|a: one batch of 26 items, four of them the creates of the new chapter cards; 3\|a with one reorder in 4\|a: one batch of 4 items), each previewed, then committed under the plan hash; `links_that_will_break` was empty in both previews. Text ran as 25 `edit_card` calls under `-Allow` (23 edits, then the two **结构** sections rebuilt from the edited tree); every written card was read back and compared with the intended text (0 differences). Before each text write a script compared the display formulas and the equation numbers of the old and new bodies, and after the pass a script compared the whole book with the backup: 859 display formulas and 41 `\tag`/`(N)` equation numbers before and after, no difference. The one equation label that left a body is `(A.7)` in the title of a Link line inside the stale **分部** list of chapter 5, which was removed (no formula). No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 530 to 534 Cards (four new chapter cards in 4\|a); no Card was deleted. Titles changed on five cards (the chapter 1 and chapter 5 cards, the A.2 test card, the two 命题 cards, now `结果：…`).

### 3\|a (empirical): the evidence spine

Main line: 数据 `3|a|a` → 设计 `|1` → 规格：基准 `|2` → 第 1 章 主要估计 `|3` → 第 2 章 识别检验 `|4` → 第 3 章 稳健性 `|5` → 第 4 章 异质性与回溯结果 `|6`. The paper has no hypothesis here (measurement). Network: chapters 1–4 under the baseline specification.

- Decision on chapter 5 (`3|a|a|7`, Appendix A.2): merged into chapter 2, not kept as a chapter. After stage 2 it held only its head and a limits card, with no estimate or test of its own; the skill asks that a chapter hold at least one estimate or test, and the four Table A.2 estimates and the fixed-effects test already stand in chapter 2, which is the identification test of the same prediction relation. Keeping it as a chapter resting on chapter 2 would have left a chapter whose table of contents points only into another chapter. The head moved as the first Side card of chapter 2 (`3|a|a|4|a`, facet `检验：附录 A.2 的预测检验`, title `检验：GTA 计数外推的补贴变化能复现 NIPO 观测的变化（附录 A.2）`) with its limits card under it (`3|a|a|4|a|b`). It keeps its display formula (the Table A.2 regression in $\alpha,\beta,\gamma_j,\delta_c$ notation) and its estimates table; it now rests on the specification `3|a|a|2|c` and on 假设 3 `3|a|a|1|c` (`依赖`), and `本章`/`本模型` became `本检验`. No Card was deleted.
- Chapter 2's Side branches are in the order of Table A.2: `4|a` the A.2 test, `4|b` the table card, then the slopes `4|c` column (1) 0.594, `4|d` (2) 0.526, `4|e` (3) 0.543, `4|f` (4) 0.407 with its test `4|f|a`. **本章估计** lists them in this order.
- The limits card 覆盖与口径 (former 3\|a\|b) moved under chapter 4 (`3|a|a|6|c`), whose **承上启下** already pointed to it; the two limits cards got one plain reference line each (`见 [[…]]`), so loose fell from 2 to 0.
- **直觉** leads added to the data, design and specification cards (one sentence each, prepended). **结构** rebuilt from the tree (main line and one level under it).

### 4\|a (formal): the intuition spine

```
4|a|a  设定 (setting part as the settings pass left it)
4|a|a|1  第 1 章：文字版本（§1）                         ← 4|a|a, 4|a|a|b
    a 机制 → a|1 结论 → a|1|a 例子 (unchanged)
4|a|a|2  第 2 章：价格指数与引力方程（§3） (new)         ← 4|a|a|c, c|1, d
    a 结果：引力方程 (6)              (proof a|a 最低到岸价格的分布 → a|a|1 价格指数 (5); former 2, 3, 4)
4|a|a|3  第 3 章：变化率形式（§3） (new)                 ← 第 2 章
    a 结果：补贴改变份额的方向 (7)    (proof a|a 变化率形式 (7); former 5, 6)
4|a|a|4  第 4 章：补贴对出口的六项分解（§3） (new)       ← 第 2 章, 第 3 章
    a 结果：出口变化的六项分解 (8)    (proof a|a 对数差 → a|a|1 六项分解 (8); a|b 解读; a|c 例子; former 7, 8, 9)
4|a|a|5  第 5 章：均衡条件（附录 A.1） (former 10)       ← 第 2 章, 4|a|a|d
    a 结果：劳动需求与劳动出清 (A.1)(A.2)  (proof a|a 部门就业 (A.1))
    b 结果：均衡由四个条件决定             (proof b|a 部门产品市场出清 (A.5))
4|a|a|6  第 6 章：变化率系统与求解（附录 A.1） (new)     ← 第 5 章, 第 3 章
    a 结果：价格指数与就业的变化 (A.7)(A.9) (proof a|a (A.6) → (A.7) → (A.8) → (A.9))
    b 结果：变化率系统只需基准份额          (proof b|a (A.10) → (A.11) → (A.12) → (A.13); b|b 传导图)
    c 求解：四个变化率变量的迭代
```

- The §3 derivations (former main-line 4\|a\|a\|2–9) left the main line: three new chapter cards hold the gravity equation, the hat form and the six-term decomposition, each result with its proof chain as a Side branch. On chapter 4 the proof chain is the result's first Side branch, before the reading and the example.
- Decision on the appendix: A.1 stays as chapters of its own, not as proof chains of §3 results. No §3 result states the equilibrium conditions (A.1), (A.2), (A.5) or the hat system (A.6)–(A.13): §3 shows only the gravity equation (as the setting `d` says), so the appendix proves results of its own (the equilibrium proposition and the hat-algebra proposition). It was split by those results into two chapters: chapter 5 (the former A.1 chapter card, equilibrium conditions) and a new chapter 6 (hat system and solution algorithm), because the former single chapter already held two parts (its **分部**) and 18 Cards. Its stale **分部** section was removed from the chapter card; the Model Card holds the parts.
- Every chapter card got **直觉** (a lead of 20+ characters), **本章结果** (pointers `结果：`, `求解：`) and **承上启下** with `依赖` lines to the setting or earlier chapter it builds on, and a 来源 line; chapters 1 and 5 keep their old 问题/概要/关联 text below the new sections (`本卡` → `本章`). This anchored chapters 1 and 5 and the four new chapters. Chapter 1's title now states its mechanism (`第 1 章：补贴吸引资源，规模经济放大降价，形成三类效应`); chapter 5's facet and title read `第 5 章：…`.
- Result facets read `结果：…` (7 Cards; the former `结果` and `命题` facets; the two 命题 titles now start with `结果：`).
- With results now ahead of their proofs, `check-settings` found eight symbols first met in a result card. The result cards now define them in one sentence ($Exp_i^j$, $\bar r_{ni}^j$, $\Delta$ in 4\|a; $Z_{in}^j$, $\nu_{in}^j$ in 6\|a; $\lambda_i^j$, $\xi_n^j$, $\kappa_n^{js}$ in 6\|b), and the notation rows point to those cards.
- The Model Card: **结构** rebuilt from the tree; **分部** rewritten (设定与直觉 from 4\|a\|a; 价格、引力方程与分解（§3） from 第 2 章; 均衡与求解（附录 A.1） from 第 5 章), since two of the old parts started off the main line; its two stale lines to its own chapters (对应 the A.1 chapter, 对照 chapter 1) were dropped. Title, 问题 and 概要 unchanged.

Network (`check-network.mjs`): 设定 → {1, 2, 5}; 2 → 3 → 4; 5 → 6.

### Gate

- `check-model.mjs 3|a`: 46 members (7 main, 39 side), loose 0, no closure, forward or unanchored fault, `self_sufficient` true (was false). `4|a`: 44 members (7 main, 37 side), loose 0, no fault, `self_sufficient` true (was false).
- `check-network.mjs`: `network: clean` for both (advice only: chapter 1 of 4\|a has no `结果：` card, right for the verbal chapter).
- `check-premises.mjs`: 3\|a 2 hits, 4\|a 1 hit, all pointers, kept with this reason:
  - 3\|a: `3|a|a|a` (变量表) and `3|a|a|e` (the A.2 sample) name the column (1) of Table A.2, not equation (1) of the baseline specification.
  - 4\|a: `4|a|a|e` (calibration **说明**) says the solution algorithm computes trade shares with (7) and points to `4|a|a|6|c`; it does not use (7), as after stage 2.
- `check-settings.mjs`: 3\|a `settings: the data tree and the specification ladder are in place`; 4\|a `settings: all in the setting part, as a dependency tree`.
- `list_broken_links` (agent): `{"broken":[]}`. Literature tree 530 → 534 Cards.
- Term relink: `{"links_added":39,"links_repaired":0,"rounds":1,"ok":true,"cards_changed":12,"cards_refused":0,"applied":true,"book":"rotunno2026industrial"}`. All checks above, `list_broken_links` and the formula comparison with the backup were rerun after it and stay as listed.

### Open

1. Step 3 card text was not redrafted: result cards keep their old body (they rest on their last proof card by `依赖`, without the `**为什么**`/`**证明**` sections of the result format), and derivation cards keep their digest format. Chapter 5 keeps its old 概要, which also summarizes the hat system that now forms chapter 6.
2. 3\|a: the A.2 test card `3|a|a|4|a` keeps its own display formula of the Table A.2 regression ($\alpha,\beta,\gamma_j,\delta_c$), beside the ladder's $\kappa,\theta_3,\gamma_j,\eta_i$ notation (stage 2 open item 1); the import stage may decide whether it should cite the ladder instead.
3. 3\|a: the 变量表 row of $\theta_3$ reads `表 A.2 第 (1) 列系数`, although $\theta_3$ is the slope in all four columns.
4. The stage 2 open items 3 (the notation row `$A$`) and 4 (the §1 verbal setting restates the calibration scope) stay.
5. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.

## Stage 4: imports and explanations

Date 2026-11-14, live box, under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent; one DeepSeek job, for the explanation refresh). Backup: `D:\zt\backups\zt-before-rotunno2026industrial-imports.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Text ran as one `edit_card` call (the premise line on `4|a|a|e`) and one scripted batch of 3 `edit_card` calls under `-Allow` (the import-line wording on 4\|a and the two stage-3 leftovers); before the batch a script compared the display formulas and `\tag` numbers of the old and new bodies, and every written card was read back and compared with the intended text (`same` on all three). After the pass a script compared the whole book with the backup: 534 Cards before and after, 859 display formulas and 48 `\tag` numbers identical, no estimate card changed. No `card_locked`, no `read_view_changed` and no failed batch occurred. Order: 3\|a, then 4\|a. Before any edit, `check` exited 0 and `plan` said `nothing to import` on both heads: the calibration named the measurement only in words (`取自第 2 节的 2015—23 年估计`).

### 3\|a (empirical: subsidy measurement)

- No import. The measurement takes nothing from another model: its data (NIPO, GTA), its specifications and its back-cast are the paper's own, and it tests no theory prediction (no `假说：` part; the paper's §2 is measurement). Its Model Card's `参照 [[rotunno2026industrial]]` (the rates are the source of the policy shock) is a plain reference to the Literature root, no false share. Tolerated 0, false shares 0.
- It now lends `3|a|a|2|d` (the back-cast) with its closure to 4\|a (`lent`).

### 4\|a (formal: the quantitative trade model)

- Decision: the theory imports the measurement block it takes as input, the case "a measurement block a theory uses as input". The calibration `4|a|a|e` item 2 takes the subsidy-rate changes (the change of $\tilde s_i^j$) from the §2 back-cast; that back-cast is the specification `3|a|a|2|d` (the coefficient 35.8 times the GTA count changes, summed over years, giving the amounts by country, sector and year). The block root is the specification, not an estimate card: the estimates `3|a|a|6|a` (China 1.8%) and `6|b` (strategic 1.8% vs 0.9%) are summaries of the back-cast series, and the model uses the whole series, not those numbers. The measure `3|a|a|f` states the same multiplication in words; the specification carries the equation and the rate $r_{iT}^j$, so it was chosen.
- `4|a|a|e` **起点** gained `依赖 [[rotunno2026industrial/3|a|a|2|d]] 的回溯：补贴率变化所用的各国、各部门、各年补贴金额来自这张规格。` (no symbol, so the notation card stays complete).
- Import: `plan` round 1 `单卡 [[3|a|a|2|d]]` (tolerated 0 → 1), round 2 upgraded it to `整块` (tolerated 1 → 0); the closure brings `3|a|a|2` (规格：基准), `3|a|a` (数据) and `3|a|a|1` (设计): 48 members (44 own + 4 shared). The line was reworded: `整块 [[rotunno2026industrial/3|a|a|2|d]]：校准的政策输入，第 2 节回溯的各国、各部门、各年补贴金额，除以 2015 年部门总产出得到补贴率变化`.
- Symbols (step 6): 4\|a keeps its own notation card; the calibration names no symbol of 3\|a, so no `沿用` line. `check-settings` clean.
- No copy: no part of 4\|a restates a 3\|a setting. False shares 0.

### Cross-book blocks

`plan` proposed no cross-book line, and no part of either model rests on another book's Card. Searched by hand: the box holds no other Caliendo–Parro / Ju et al. trade model or Fréchet-productivity block (becker2025division/3\|a\|a\|13\|d\|a has a relative gravity equation (S.10), a derivation of that model's own, not a block 4\|a restates). The cross-book Links (4\|a `对照` acemoglu2025nobel/3\|h\|e\|1 and atkinson2015lectures/17\|b\|a\|b\|4; chapter 5 `对照` gomezcram2026government/3\|i\|a\|13; 3\|a `对应` brandimarti2026reorganizing/10\|a\|7) are comparisons, kept; the 3\|a one is for the evidence dialogue.

### Stage-3 leftovers fixed (no formula change)

1. Chapter 5 `4|a|a|5`: the 问题 dropped its second sentence (whether the system can be written in change form, which is chapter 6's question); the 概要 sentence on (A.6)–(A.13) became `同一系统的变化率形式 (A.6)–(A.13) 不在本章，归第 6 章 [[rotunno2026industrial/4|a|a|6]]。` The inline $\theta^j,\psi^j,\beta_i^j,\gamma_i^{sj}$ of the old sentence went with it (no display formula).
2. 3\|a 变量表 `3|a|a|a`: the $\theta_3$ row now reads `NIPO 观测变化的斜率，表 A.2 四列共同的系数` (was `表 A.2 第 (1) 列系数`). `check-premises` on 3\|a went from 2 to 1 hit.

### Explanation refresh (step 8)

- `explain.mjs plan`: 4\|a 4 Derivation parts in 1 job; 3\|a `nothing to explain`.
- One DeepSeek conversation (`opencode-go/deepseek-v4.1-flash`, effort high, the box's drive model; OpenCode Go account Primary, 8 requests, 131 s, 1 turn, context 66k of 1M; tools: 4 pwsh, 2 read, 1 write; only `out/explain-01.json` was written; no approval was asked): answer `{"done":4,"blocked":0}`. Cost: OpenCode Go subscription, the Harness ledger books 0 CNY; the 2026-11-14 opencode-go day line went from 5,000 to 5,008 calls during the job (cache miss +66,021, cached read +385,152, output +10,855 tokens), matching the job's 8 requests. Conversation 219a3693 deleted, workspace removed. Conversation 3b1d187b was not touched.
- `apply`: `applied 4, refused 0, blocked 0, follow-up jobs 0`. `finish` 4\|a: `explained 4 part(s); Term links changed 3 Card(s), second plan 0; left 0; refused 0` (exit 0); parts 4\|a\|a\|6\|a\|a and \|1–\|3 (the chain (A.6)–(A.9)). `finish` 3\|a: `explained 0 part(s) … left 0; refused 0` (exit 0).

### Gate

- `imports.mjs check`: exit 0 on 3\|a and 4\|a; `plan`: `nothing to import` on both.
- `get_model`: `self_sufficient: true` on both; tolerated 0 and false shares 0 on both; 3\|a 46 parts, lends `3|a|a|2|d` to 4\|a; 4\|a 48 parts, one `整块` import (closure 3\|a\|a\|2, 3\|a\|a, 3\|a\|a\|1).
- `explain.mjs finish`: exit 0 on both (`left 0; refused 0`); a later `plan`: `nothing to explain` on both.
- `check-model.mjs`: 3\|a 46 members `{"main":7,"side":39}`, `relations {"refers":68,"rests-on":23,"illustrates":5}; loose 0; self_sufficient true`; 4\|a 48 members `{"shared":4,"main":7,"side":37}`, `relations {"refers":37,"rests-on":72,"illustrates":3}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` on both (advice only: chapter 1 of 4\|a has no result card, as at stage 3).
- `check-premises.mjs`: 1 hit each, both pointers kept: 3\|a `3|a|a|e` names column (1) of Table A.2, not equation (1); 4\|a `4|a|a|e` **说明** names (7) for the solution algorithm and points to `4|a|a|6|c` (as at stage 3).
- `check-settings.mjs`: 3\|a `settings: the data tree and the specification ladder are in place`; 4\|a `settings: all in the setting part, as a dependency tree`.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"links_added":4,"applied":true,"cards_refused":0,"links_repaired":0,"book":"rotunno2026industrial","ok":true,"cards_changed":2,"rounds":1}`. All checks above, `list_broken_links` and the formula comparison with the backup were rerun after it and stay as listed.

### Open

1. Denominator: the back-cast `3|a|a|2|d` defines the rate $r_{iT}^j$ over sector value added (as Figure 3), while the calibration `4|a|a|e` item 2 divides the back-cast amounts by 2015 gross output. Both follow the paper; the import takes the amounts, and the import line names the 2015-output division. Not changed.
2. The measure `3|a|a|f` and the specification `3|a|a|2|d` state the same multiplication (35.8 times the GTA count changes, summed); left as stage 2 left them.
3. Stage-3 open items 1 (result and derivation text not redrafted; chapter 5 keeps its old **关联** line naming the Literature notes 9\|a\|4–5 on the hat form and the decomposition), 2 (the A.2 test card's own $\alpha,\beta,\gamma_j,\delta_c$ regression) and 4 (stage-2 items 3 and 4) stay. The A.2 test card was not changed here: it imports nothing, and citing the ladder instead of its own equation would delete a display formula.
4. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.
