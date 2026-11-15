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
