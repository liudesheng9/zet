# levine2017smart: post-digest sequence

Owner: Xingtong Chen

Levine & Rubinstein (2017), *Smart and Illicit: Who Becomes an Entrepreneur and Do They Earn More?* (*Quarterly Journal of Economics*, 132(2), 963–1018).

- Live box: `D:\zt` (agent workspace, tree `levine2017smart` unlocked throughout).
- Pre-Stage-1 backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3`.

## Stage 1: fold (2026-11-27)

### Decision table

Initial survey: 7 empirical Model Cards (`4|a`, `5|a`, `5|b`, `6|a`, `6|b`, `7|a`, `7|b`), all level 3 (`empirical`). Under `docs/empirical-models-proposal.md` §9.1, empirical cards sharing the same estimand on the same dataset (including identification/selection-bias refinements, heterogeneity splits, external-validity robustness, and quantile estimator families) fold into one empirical Model Card; distinct datasets or distinct estimands stay separate.

| Initial Model Card | Section / Table | Initial Parts | Decision | Surviving Head | Reason |
| --- | --- | ---: | --- | --- | --- |
| `levine2017smart/4|a` | §III, Eq. (1), Tables V–VI | 26 | Own model | `levine2017smart/4|a` | Distinct dataset (1995–2012 CPS 2-year matched panel, $N=230,330$) and estimand: lagged salaried job task requirements predicting transition to incorporated/unincorporated self-employment. |
| `levine2017smart/5|a` | §IV.A, Eq. (2), Table VII | 31 | Own model | `levine2017smart/5|a` | Distinct dataset (1982–2012 NLSY79 full employed panel, $N=125,166$) and estimand: pre-labor-market cognitive/noncognitive and family traits predicting adult employment type. |
| `levine2017smart/5|b` | §IV.B, Table VIII | 32 | Own model | `levine2017smart/5|b` | Distinct estimand and merged sample (NLSY79 white males salaried at $t-2$ merged with CPS Table VI industry task scores): within-individual change in *industry task requirements* of the new business across 4 trait subsamples. |
| `levine2017smart/6|a` | §V.A, Eqs. (3)–(4), Table IX | 43 | Surviving head | `levine2017smart/6|a` | Anchor model for Section V ("Do Entrepreneurs Earn More?"): NLSY79 white male full-time panel ($N=23,657$ levels; $N=17,479$ 2-year FD) estimating annual and hourly earnings gains of incorporated/unincorporated self-employment vs. salaried work (OLS & median, levels & FD, with/without individual FE and linear trend). |
| `levine2017smart/6|b` | §V.B, Eqs. (5)–(7), Table X | 38 | Fold into `6|a` | `levine2017smart/6|a` | Same estimand ($\beta_I, \beta_U$) and exact same NLSY79 white male panel ($N=23,657$, 3,553 spells; Table X cols 1/5/9/13 replicate Table IX cols 2/6/10/14): adds spell-start legal form, inverse-spell-length weighting, and first-spell classification to address selection and survivorship bias. |
| `levine2017smart/7|a` | §V.C, Eq. (8), Table XI | 25 | Fold into `6|a` | `levine2017smart/6|a` | Same estimand and exact same NLSY79 white male 2-year FD sample ($N=17,479 = 4,210 + 13,269$): Eq. (8) is the FD of Eq. (7) split by "smart and illicit" trait heterogeneity, plus nonroutine cognitive industry DiD external-validity robustness check. |
| `levine2017smart/7|b` | §V.D, Eq. (3), Figures I–II | 19 | Fold into `6|a` | `levine2017smart/6|a` | Same estimand (Eq. 3) and exact same NLSY79 white male panel ($N=23,657$): quantile regression estimator family (Q10–Q90, without/with individual FE) extending Table IX cols (5)–(6) (Q50) across the full annual earnings distribution, plus Eq. (7)/hourly robustness and CV risk comparison. |

### Fold actions

All writes executed through `zt_mutex.py` on `D:\zt` following `zt-model-fold/SKILL.md` Step 3 (one head at a time, atomic `tidy` batches, no math changed, no Links deleted, no Idea-note children on any folded head, no `**引入**` sections on folded heads):

1. **`levine2017smart/6|b` → `levine2017smart/6|a|a|5`** (`facet: "第 3 章：持续期选择偏差修正"`, `emphasis: ""`, `level: ""`):
   - Stripped `**结构**` block; parked `6|b|a`, `6|b|b` as Side successors of `6|a`; masked 10 links to `6|a` parts; retyped `6|b`; restored body; moved `6|b` `alone` after `6|a|a|4` and restored parked subtrees as `6|a|a|5|a`, `6|a|a|5|b`.
2. **`levine2017smart/7|a` → `levine2017smart/6|a|a|6`** (`facet: "第 4 章：特质异质性与行业双重差分"`, `emphasis: ""`, `level: ""`):
   - Stripped `**结构**` block; parked `7|a|a`, `7|a|b` as Side successors of `6|a`; masked 10 links to `6|a` parts; retyped `7|a`; restored body; moved `7|a` `alone` after `6|a|a|5` and restored parked subtrees as `6|a|a|6|a`, `6|a|a|6|b`.
3. **`levine2017smart/7|b` → `levine2017smart/6|a|a|7`** (`facet: "第 5 章：分位数收入分布与离散度"`, `emphasis: ""`, `level: ""`):
   - Stripped `**结构**` block; parked `7|b|a`, `7|b|b` as Side successors of `6|a`; masked 8 links to `6|a` parts; retyped `7|b`; restored body; moved `7|b` `alone` after `6|a|a|6` and restored parked subtrees as `6|a|a|7|a`, `6|a|a|7|b`.

### Surviving heads (4 empirical models)

| Surviving Head | Level | Parts | Summary |
| --- | --- | ---: | --- |
| `levine2017smart/4|a` | empirical | 26 | CPS 2-year matched panel: lagged salaried job task requirements predicting incorporated/unincorporated self-employment (§III, Eq. 1, Tables V–VI). |
| `levine2017smart/5|a` | empirical | 31 | NLSY79 employed panel: pre-market cognitive/noncognitive and family traits predicting adult employment type (§IV.A, Eq. 2, Table VII). |
| `levine2017smart/5|b` | empirical | 32 | NLSY79 white males + CPS industry task scores: within-individual industry task sorting of new businesses by trait group (§IV.B, Table VIII). |
| `levine2017smart/6|a` | empirical | 128 | NLSY79 white male full-time earnings panel: levels/FD OLS & median regressions, spell-level selection/survivorship corrections, smart-and-illicit heterogeneity & industry DiD, and Q10–Q90 quantile distribution (§V, Eqs. 3–8, Tables IX–XI, Figs. I–II). |

### Gate results

- `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs levine2017smart`: **PASS** — 4 Model Cards (`4|a`, `5|a`, `5|b`, `6|a`), 4 candidate groups, 0 structural joins.
- `zt-check --model '<head>' --stage fold`: **PASS** (exit 0 on all 4 surviving heads):
  - `levine2017smart/4|a`: 26 parts (`main: 5, off: 1, side: 20`), `self_sufficient true`, `evidence_bound true`, `stage(fold) pass`.
  - `levine2017smart/5|a`: 31 parts (`main: 5, off: 1, side: 25`), `self_sufficient true`, `evidence_bound true`, `stage(fold) pass`.
  - `levine2017smart/5|b`: 32 parts (`main: 5, off: 1, side: 26`), `self_sufficient true`, `evidence_bound true`, `stage(fold) pass`.
  - `levine2017smart/6|a`: 128 parts (`main: 8, off: 1, side: 119`), `stage(fold) pass` (expected deferred notices only: 3 Stage 2 `spec_unbound` on `6|a|a|5|a|2`, `6|a|a|6|a|2`, `6|a|a|7|a|2`, and 3 Stage 3 `unanchored` on folded heads `6|a|a|5`, `6|a|a|6`, `6|a|a|7`).
- `list_broken_links` (agent workspace): **PASS** — 0 broken links total, 0 in `levine2017smart/`.
- Card count in `levine2017smart`: **389** (unchanged from 389 pre-fold).
- `zt-digest term-link -Step relink -CitationKey levine2017smart` (and `-PlanOnly`): **PASS** — `cards_changed: 0`, `links_added: 0`, `links_repaired: 0`, `cards_refused: 0`.
- Next stage: hand `levine2017smart/4|a`, `levine2017smart/5|a`, `levine2017smart/5|b`, and `levine2017smart/6|a` to `zt-model-settings` (Stage 2).

## Stage 2: settings (2026-11-27)

- Live box: `D:\zt` (agent workspace, all writes serialized through `zt_mutex.py`).
- Pre-Stage-2 backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`.
- Display formulas verified byte-for-byte before every write (0 display formulas in any of the 9 dissolved cards; all surviving formulas preserved verbatim).

### Per-model changes

- **`levine2017smart/4|a` (empirical, 26 → 26 parts)**:
  - Fixed `start` fault on `4|a|a|b` (`测量：任务要求与行业指标`): added one-line summary, `**起点**` (`- 依赖 [[levine2017smart/4|a|a]]：指标基于 CPS 与 DOT 数据构造。`), and `**内容**`; dropped redundant `见 [[levine2017smart/4|a|a]]。`; preserved display formula $\overline{NR}_{k,m}$ byte-for-byte.
- **`levine2017smart/5|a` (empirical, 31 → 31 parts)**:
  - Fixed `start` fault on `5|a|a|b` (`测量：早年认知非认知特质与家庭插补`): promoted summary sentence to line 1, added `**起点**` (`- 依赖 [[levine2017smart/5|a|a]]：…`) and `**内容**`, dropped redundant `见` line.
- **`levine2017smart/5|b` (empirical, 32 → 32 parts)**:
  - Fixed `start` fault on `5|b|a|b` (`测量：行业任务指标与特质分组`): promoted summary sentence to line 1, added `**起点**` (`- 依赖 [[levine2017smart/5|b|a]]：…`) and `**内容**`, dropped redundant `见` line.
- **`levine2017smart/6|a` (empirical, folded from `6|a` + `6|b` + `7|a` + `7|b`, 128 → 119 parts)**:
  - **Tree dissolution (`apply_batch`, 28 items, 70 cards moved, 9 deleted `into`, `links_that_will_break: []`)**:
    - Moved 3 folded measure cards (`6|a|a|5|a|b`, `6|a|a|6|a|b`, `6|a|a|7|a|b`) under `6|a|a` as `6|a|a|c`–`e`.
    - Moved 7 folded identifying assumption cards (`5|a|1|a..c`, `6|a|1|a..b`, `7|a|1|a..b`) under `6|a|a|1` (`设计`) as `6|a|a|1|e`–`k`.
    - Moved the 3 folded specification subtrees onto `6|a|a|2`'s specification ladder under the exact specification each extends: `5|a|2` (Eq. 5 / Table X col 1) → `6|a|a|2|a|a` (under `6|a|a|2|a`, Table IX col 2); `6|a|2` (Eq. 8 / Table XI col 1) → `6|a|a|2|c|b|b` (under `6|a|a|2|c|b`, Table IX col 7); `7|a|2` (Q10–Q90 no-FE) → `6|a|a|2|c|c` (under `6|a|a|2|c`, Table IX col 5).
    - Promoted folded sub-chapter Direct chains (`5|a|3..4`, `6|a|3..4`, `7|a|3..4`) directly under `6|a|a|5`, `6|a|a|6`, `6|a|a|7` and reordered them ahead of each chapter's `局限` card (`|a`, `|a|1` before `|b`).
    - Deleted the 9 redundant duplicate cards `into` the surviving `6|a` cards: `5|a|a`, `6|a|a`, `7|a|a` → `6|a|a|a` (`变量表`); `5|a|1`, `6|a|1`, `7|a|1` → `6|a|a|1` (`设计`); `5|a`, `6|a`, `7|a` → `6|a|a` (`数据`).
  - **Text edits (`edit_card`, 17 parts + head `6|a`)**:
    - Merged sample details (3,553 spells, Table XI trait subsamples 4,210/13,269, Figs. I–II age note) into `6|a|a` (`数据`).
    - Merged all 26 new variable/symbol rows (`E_{its}`, `E_{its}^H`, `I_{its}`, `I_{is}`, `I_i^{(1)}`, `U_{its}`, `U_{is}`, `U_i^{(1)}`, `X_{its}`, `\varepsilon_{its}`, `\epsilon_{its}`, `\delta_0`–`\delta_3`, `u_{its}`, `\gamma_0`–`\gamma_4`, `SE_{it}`, `NRC_{it}`, `v_{its}`, `Q_\tau`, `\tau`) into `6|a|a|a` (`变量表`, 41 rows total, 0 `uncovered`, 0 `unused`).
    - Formatted `6|a|a|b`–`e` with one-line summary and `**起点**` (`- 依赖 [[levine2017smart/6|a|a]]：…`).
    - Merged design notes into `6|a|a|1` (`设计`) listing all 11 assumptions, and renumbered `6|a|a|1|e`–`k` to `假设 5`–`假设 11`.
    - Converted `6|a|a|2|a|a`, `6|a|a|2|c|b|b`, `6|a|a|2|c|c` from duplicate `规格：基准` facets to variant facets with `依赖 [[<parent>]]` and `**相对基准的改动**` (display formulas (5), (8), and $Q_\tau$ preserved byte-for-byte).
    - Rebuilt `levine2017smart/6|a` `**结构**` section from build order (119 parts).

### Gate results (post term-relink)

- `zt-digest term-link -Step relink -CitationKey levine2017smart`: **PASS** — `cards_changed: 4, links_added: 8, links_repaired: 0, cards_refused: 0`; `-PlanOnly` rerun: `cards_changed: 0, links_added: 0, links_repaired: 0`.
- `zt-check --model '<head>' --settings`: **PASS** (exit 0 on all 4 models — `settings: data tree and specification ladder are clean`).
- `zt-check --model '<head>' --stage settings`: **PASS** (exit 0 on all 4 models):
  - `levine2017smart/4|a`: 26 parts (`main: 5, off: 1, side: 20`), `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`.
  - `levine2017smart/5|a`: 31 parts (`main: 5, off: 1, side: 25`), `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`.
  - `levine2017smart/5|b`: 32 parts (`main: 5, off: 1, side: 26`), `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`.
  - `levine2017smart/6|a`: 119 parts (`main: 8, off: 1, side: 110`), `evidence_bound true`, `stage(settings) pass` (only expected Stage 3 deferred `unanchored` notices on folded chapters `6|a|a|5`, `6|a|a|6`, `6|a|a|7`).
- `list_broken_links` (agent workspace): **PASS** — 0 broken links total, 0 in `levine2017smart/`.
- Card count in `levine2017smart`: **380** (389 − 9 dissolved duplicate cards).
- Next stage: hand `levine2017smart/4|a`, `levine2017smart/5|a`, `levine2017smart/5|b`, and `levine2017smart/6|a` to `zt-model-restructure` (Stage 3).

## Stage 3: restructure (2026-11-27)

- Live box: `D:\zt` (agent workspace, all writes serialized through `zt_mutex.py`).
- Pre-Stage-3 backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`.
- Integrity: all 61 display-formula cards across the 4 models verified byte-for-byte identical before and after edits (`diffs: 0`); no empirical estimate number altered; no Link or `依赖` dropped.

### Per-model changes

- **`levine2017smart/4|a` (empirical, 26 → 26 parts, 3 cards edited, 0 moves)**:
  - Added `**直觉**` leads (>=20 chars) on `4|a|a|1` (`设计`) and `4|a|a|2` (`规格：基准`).
  - Anchored off-spine limits card `4|a|b` with plain reference `见设计 [[levine2017smart/4|a|a|1]]。` (`loose` 1 → 0).
- **`levine2017smart/5|a` (empirical, 31 → 31 parts, 2 cards edited, 0 moves)**:
  - Added `**直觉**` leads (>=20 chars) on `5|a|a|1` (`设计`) and `5|a|a|2` (`规格：基准`).
- **`levine2017smart/5|b` (empirical, 32 → 32 parts, 3 cards edited, 0 moves)**:
  - Added `**直觉**` leads (>=20 chars) on `5|b|a|1` (`设计`) and `5|b|a|2` (`规格：基准`).
  - Cleared 4 `R22` bare-LaTeX faults on `5|b|a|2` (`$I_{it}$`, `$U_{it}$`, `$\mu_i$`, `$\lambda_t$`).
  - Anchored off-spine limits card `5|b|b` with plain reference `见设计 [[levine2017smart/5|b|a|1]]。` (`loose` 1 → 0).
- **`levine2017smart/6|a` (empirical, 119 → 113 parts, 1 atomic `tidy` batch + 12 cards edited)**:
  - **Inner-chapter dissolution (`apply_batch`, 35 cards moved, 6 deleted `into`, `links_that_will_break: []`, 178 link macros rewritten)**:
    - Dissolved the 6 redundant inner chapter wrappers inside folded chapters `6|a|a|5..7` (`5|a`, `5|a|1` → `6|a|a|5`; `6|a`, `6|a|1` → `6|a|a|6`; `7|a`, `7|a|1` → `6|a|a|7`) and promoted all tables, figures, estimates, and tests directly as Side branches ahead of each chapter's limits card (`5|a..m`, `6|a..l` with `6|e|a`, `7|a..i`).
  - **Chapter anchoring & card-form cleanup**:
    - Rewrote `6|a|a|5`, `6|a|a|6`, and `6|a|a|7` as standard empirical chapter cards (`第 3 章`–`第 5 章`) with `依赖` links to `6|a|a|3` and their respective specification cards, `**直觉**` leads (>=20 chars, <=80 chars), `**本章估计**`, and `**承上启下**` (clearing all 3 `unanchored` faults).
    - Added `**直觉**` leads (>=20 chars) on `6|a|a` (`数据`), `6|a|a|1` (`设计`), and `6|a|a|2` (`规格：基准`).
    - Anchored all 3 chapter limits cards (`6|a|a|5|m`, `6|a|a|6|l`, `6|a|a|7|i`) to their chapter cards (`loose` 3 → 0).
    - Cleared all 11 blocking `--skip-shared` faults: `6|a|a|1` (`R3` split, `R4` `并非由做大后注册主导`, `R13` trimmed to <=24 sentences, `R2` <=80 chars), and `R22` bare LaTeX on `6|a|a|2`, `6|a|a|2|c|b|b`, and `6|a|a|2|d`.
    - Rebuilt `**结构**` (113 owned parts in build order) and added `**分部**` (4 parts covering `6|a|a`..`6|a|a|7`) on head `levine2017smart/6|a`.

### Gate results (post term-relink)

| Model | Parts (main/side/off) | loose | self_sufficient | `--stage restructure` | `--network` | `--settings` | `--skip-shared` | `--premises` |
| --- | --- | ---: | --- | --- | --- | --- | --- | --- |
| `levine2017smart/4|a` | 26 (5/20/1) | 0 | true | pass (rc=0) | clean (6 blocks, 4 layers, rc=0) | clean (rc=0) | faults: 0 (rc=0) | 1 pointer (0 hidden uses) |
| `levine2017smart/5|a` | 31 (5/25/1) | 0 | true | pass (rc=0) | clean (6 blocks, 4 layers, rc=0) | clean (rc=0) | faults: 0 (rc=0) | 5 Table VII col pointers (0 hidden uses) |
| `levine2017smart/5|b` | 32 (5/26/1) | 0 | true | pass (rc=0) | clean (6 blocks, 4 layers, rc=0) | clean (rc=0) | faults: 0 (rc=0) | none (rc=0) |
| `levine2017smart/6|a` | 113 (8/104/1) | 0 | true | pass (rc=0) | clean (9 blocks, 4 layers, rc=0) | clean (rc=0) | faults: 0 (rc=0) | 26 symbol-table/col/pointer hits (0 hidden uses) |

- `--premises` audit (all 0 hidden forward uses):
  - `4|a`: 1 hit in `4|a|a` (`怎样进入模型：规格：基准用该面板估计式 (1)` — forward pointer to eq. (1)).
  - `5|a`: 5 hits in `5|a|a|1` and `5|a|a|1|c` naming Table VII columns `(1)`, `(2)`, `(5)`, `(9)`.
  - `5|b`: 0 hits.
  - `6|a`: 26 hits — 6 variable-table rows (`6|a|a|a`), 1 superscript `$I_i^{(1)}$` (`6|a|a|c`), 1 forward spec pointer (`6|a|a|d` eq. (7)), 4 design roadmap pointers (`6|a|a|1` eqs. (3)–(8), (7), `$I_i^{(1)}$`), 11 assumption/footnote pointers & Table X column numbers (`6|a|a|1|b`, `1|e`, `1|f`, `1|g`, `1|k`), and 3 Table IX/X column `(1)` + eq. (6) decomposition pointers (`6|a|a|2`, `6|a|a|2|a|a`).
- `zt-digest term-link -Step relink -CitationKey levine2017smart`: **PASS** (18 links added across 9 cards across passes; final `-PlanOnly`: `cards_changed: 0, links_added: 0, links_repaired: 0, cards_refused: 0`).
- `list_broken_links` (agent workspace): **PASS** — `[]` (0 broken links total, 0 in `levine2017smart/`).
- Card count in `levine2017smart`: **374** (380 − 6 dissolved inner chapter cards).
- Next stage: hand `levine2017smart/4|a`, `levine2017smart/5|a`, `levine2017smart/5|b`, and `levine2017smart/6|a` to `zt-model-import` (Stage 4).

## Stage 4: imports and explanations (2026-11-27)

- Live box: `D:\zt` (agent workspace, all writes serialized through `zt_mutex.py`).
- Pre-Stage-4 backup: `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3`.
- Order: lenders `4|a`, `5|a` first, then borrowers `5|b`, `6|a`.
- Baseline (`imports.mjs check` / `plan`): all 4 models exited 0 with `nothing to import`, 0 legacy shares, 0 false shares, 0 possible copies, 0 tolerated premises; shared measurement dependencies across models had been restated inline rather than wired via `依赖` + `**引入**`.

### Per-model changes

- **`levine2017smart/4|a` (empirical, lender — no imports)**:
  - CPS 1995–2012 matched panel (`4|a|a`) and 1991 DOT task requirements / Table VI hours-weighted industry task score $\overline{NR}_{k,m}$ (`4|a|a|b`). No imports needed; off-spine limits card `4|a|b` keeps its `参照 [[levine2017smart/5|a]]` comparison (not a premise or false share).
  - Now lends `整块 [[levine2017smart/4|a|a|b]]` (closure: `4|a|a|b`, `4|a|a`, 2 cards) to `5|b` and `6|a`.
- **`levine2017smart/5|a` (empirical, lender — no imports)**:
  - NLSY79 1982–2012 employed panel (`5|a|a`) and 1979–1980 pre-labor-market trait measures (AFQT, Rosenberg self-esteem, Rotter locus of control, 20-item standardized Illicit index, `5|a|a|b`). Distinct dataset and measures from `4|a`; no imports needed.
  - Now lends `整块 [[levine2017smart/5|a|a|b]]` (closure: `5|a|a|b`, `5|a|a`, 2 cards) to `5|b` and `6|a`.
- **`levine2017smart/5|b` (empirical, borrower — 2 `整块` imports, +4 shared cards)**:
  - Wired `5|b|a|b` (`测量：表 VI 行业工时加权任务分与四特质子样本界定`) `**起点**` to rest on `4|a|a|b` (Table VI hours-weighted CPS/DOT industry task scores) and `5|a|a|b` (1980 AFQT and 20-item standardized Illicit index) (`依赖 …（经 **引入**）`), and tightened `**内容**` to build on the imported constructions rather than restating them verbatim.
  - `imports.mjs plan/apply` round 1 wrote `单卡 [[4|a|a|b]]` and `单卡 [[5|a|a|b]]` (`tolerated 0 -> 2`); round 2 upgraded both to `整块 [[4|a|a|b]]` and `整块 [[5|a|a|b]]` (`tolerated 2 -> 0`, bringing in `4|a|a` and `5|a|a`); round 3 `plan`: `nothing to import`. Polished both `**引入**` line descriptions in `5|b`.
- **`levine2017smart/6|a` (empirical, borrower — 2 `整块` imports, +4 shared cards)**:
  - Wired `6|a|a|b` (`测量：水平值收入与控制变量消去规则`) `**起点**` to rest on `5|a|a|b` (the 4 pre-labor-market trait controls in Eq. (3)), and wired `6|a|a|d` (`测量：特质分组、持续期首年形式与非常规认知行业`) `**起点**` to rest on `5|a|a|b` (AFQT and Illicit index for the "smart and illicit" split) and `4|a|a|b` (Table VI DOT Analytical & DCP industry scores for $NRC_{it}$).
  - Decision: `6|a` does not import from `5|b` — its $NRC_{it}$ indicator draws directly on `4|a|a|b`'s Table VI industry task scores and its trait split draws on `5|a|a|b`, while `6|a|a|6|i` merely notes footnote 29's 4-way robustness check analogous to Table VIII without resting on `5|b`'s estimates.
  - `imports.mjs plan/apply` round 1 wrote `单卡 [[5|a|a|b]]` and `单卡 [[4|a|a|b]]` (`tolerated 0 -> 2`); round 2 upgraded both to `整块 [[5|a|a|b]]` and `整块 [[4|a|a|b]]` (`tolerated 2 -> 0`, bringing in `5|a|a` and `4|a|a`); round 3 `plan`: `nothing to import`. Polished both `**引入**` line descriptions in `6|a`.
- **Symbols (step 6)**: Neither `5|b` nor `6|a` imports a notation card (`4|a|a|a` and `5|a|a|a` are outside the closures of `4|a|a|b` and `5|a|a|b`); both keep their complete local `变量表` (`--settings` clean, no `沿用` line needed).
- **Explanations (step 8)**: `zt-digest model-explain -Step plan -Force` on all 4 models returned `nothing to explain` (`parts: 0, jobs: []`, no Derivation parts, no DeepSeek job).

### Final Stage 4 gate (post term-relink)

| Model | Imports | Members (main/side/shared/off) | Tolerated | `imports check` | `--stage final` | `--settings` | `--network` | `--skip-shared` |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| `levine2017smart/4|a` | none (lends `4\|a\|a\|b` to `5\|b`, `6\|a`) | 26 (5/20/0/1) | 0 | pass (rc=0) | pass (`self_sufficient true`, `evidence_bound true`) | clean (rc=0) | clean (6 blocks, 4 layers) | `faults: 0` (rc=0) |
| `levine2017smart/5|a` | none (lends `5\|a\|a\|b` to `5\|b`, `6\|a`) | 31 (5/25/0/1) | 0 | pass (rc=0) | pass (`self_sufficient true`, `evidence_bound true`) | clean (rc=0) | clean (6 blocks, 4 layers) | `faults: 0` (rc=0) |
| `levine2017smart/5|b` | `整块 4\|a\|a\|b`, `整块 5\|a\|a\|b` | 36 (5/26/4/1) | 0 | pass (rc=0) | pass (`self_sufficient true`, `evidence_bound true`) | clean (rc=0) | clean (8 blocks, 5 layers, 2 `共用` shelves) | `faults: 0` (rc=0) |
| `levine2017smart/6|a` | `整块 5\|a\|a\|b`, `整块 4\|a\|a\|b` | 117 (8/104/4/1) | 0 | pass (rc=0) | pass (`self_sufficient true`, `evidence_bound true`) | clean (rc=0) | clean (11 blocks, 5 layers, 2 `共用` shelves) | `faults: 0` (rc=0) |

- `zt-digest term-link -Step relink -CitationKey levine2017smart` (under `zt_mutex.py`): **PASS** — `cards_changed: 3, links_added: 5, links_repaired: 0, cards_refused: 0`; `-PlanOnly` rerun: `cards_changed: 0, links_added: 0, links_repaired: 0`.
- `list_broken_links`: **PASS** — `[]` (0 broken links total, 0 in `levine2017smart/`).
- Card count in `levine2017smart`: **374** (unchanged).
- Next stage: `zt-evidence-dialogue` (Stage 5) on empirical models `4|a`, `5|a`, `5|b`, `6|a`, then `zt-literature-organize` (Stage 6).

## Stage 5: evidence dialogue

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; judged by Gemini 4 Neon Medium swarm workers, 0 DeepSeek cost). Backup: `D:\zt\backups\zt-before-iof10-stage5-dialogue-2026-11-27.sqlite3`.

| Model | Unique pairs judged | Same estimand (印证 / 反驳 / 对照) | Cards written | Check |
|---|---|---|---|---|
| `4|a` | 10 | 0 (0 / 0 / 0) | 0 | `{"head":"levine2017smart/4|a","broken":0,"proposed_again":0,"ok":true}` |
| `5|a` | 0 | 0 (0 / 0 / 0) | 0 | 0 candidate pairs |
| `5|b` | 18 | 0 (0 / 0 / 0) | 0 | `{"head":"levine2017smart/5|b","broken":0,"proposed_again":0,"ok":true}` |
| `6|a` | 0 | 0 (0 / 0 / 0) | 0 | `{"head":"levine2017smart/6|a","broken":0,"proposed_again":0,"ok":true}` |

- Term relink: not needed (0 Cards edited).

## Stage 6: literature organize (2026-11-27)

- Live box: `D:\zt` (agent workspace, tree `levine2017smart` unlocked throughout; all writes serialized through `zt_mutex.py`).
- Pre-Stage-6 backup: `D:\zt\backups\zt-before-iof10-stage6-organize-2026-11-27.sqlite3`.
- No DeepSeek job ran (0 DeepSeek cost).

### Source divisions

From `D:\zt-digest\levine2017smart\book.zh.md` (body from 一、引言 through 六、结论; `# 补充材料` is a one-line notice pointing to QJE's Online Appendix website, already noted in the conclusion Card):
1. `一、引言` (no subdivisions)
2. `二、数据与描述性统计` (`II.A` CPS 数据, `II.B` NLSY79：基本情况, `II.C` NLSY79：认知与非认知特质, `II.D` 公司化是否反映了基于企业成功的“事后”选择？, `II.E` 工作任务要求)
3. `三、公司制与非公司制自雇者之间的差异` (no subdivisions; Table VI is introduced in the closing paragraph of §III)
4. `四、谁会成为企业家？` (`IV.A` 基于认知与非认知特质的就业类型选择, `IV.B` 特质、就业类型与工作任务要求)
5. `五、企业家赚得更多吗？` (untitled opening + full-section empirical model + `V.A` 年收入与小时收入, `V.B` 进出白领与自雇的自我选择, `V.C` 哪些企业家赚得更多？, `V.D` 风险与收入离散度)
6. `六、结论` (no subdivisions)
7. `术语表` (Term list, 27 Cards)
8. `参考文献` (Cited list, 50 Cards)

### Final tree (top line and divisions)

```
1  一、引言 (old 1, 14)                         1|c..1|c|10 §I chain · 1|d structure Card
2  二、数据与描述性统计 (old 2, rewrite, 24)      2|a II.A (old 2|a..3 + 2 findings) · 2|b II.B (old 2|a|4..5 + 2 findings)
                                                2|c II.C (old 2|a|6..9) · 2|d II.D (old 3|a..1 + finding) · 2|e II.E (old 3 trimmed + old 3|a|2..3 + finding)
3  三、公司制与非公司制自雇者之间的差异 (old 4, 36) 3|a model (old 4|a, 27) · 3|b..3|b|3 §III chain (3|b|3|a Table VI from 5|c|a) · 3|c..3|e 3 findings
4  四、谁会成为企业家？ (old 5, rewrite, 77)      4|a IV.A (old 5|c..4, model 4|a|a [old 5|a, 32], 2 findings) · 4|b IV.B (old 5|c|5..6, model 4|b|a [old 5|b, 33], finding)
5  五、企业家赚得更多吗？ (old 6, rewrite, 144)    5|a 开篇 (old 6|c) · 5|b model (old 6|a, 114) · 5|c V.A (old 6|c|1..4 + 4 findings)
                                                5|d V.B (old 6|c|5..8) · 5|e V.C (old 7 trimmed + old 7|c..2 + 3 findings) · 5|f V.D (old 7|c|3..4 + 2 findings)
6  六、结论 (old 7|c|5, 1)
7  术语表 (Term list, from 1|b, 27)
8  参考文献 (Cited list, from 1|a, 50)
```

### Moves, crossers, and text edits

- **4 tail-to-head atomic batches** (`batch.mjs --commit`, all `complete: true, refusals: [], breaks: 0`):
  - `ops1_sec5_sec6.txt` (§V + §VI: 144 renamed, 656 link rewrites)
  - `ops2_sec4_sec3.txt` (§IV + Table VI to §III: 77 renamed, 629 link rewrites)
  - `ops3_sec2.txt` (§II.A–II.E: 276 renamed, 1,176 link rewrites)
  - `ops4_lists.txt` (Term list `7` and Cited list `8` to tail: 77 renamed, 611 link rewrites)
- **Crossers settled**:
  - `old 1` held §I plus both reference lists: `1|b` (Term) and `1|a` (Cited) moved to `7` and `8`; structure Card `1|d` stays under §I.
  - `old 2` interleaved `II.A`–`II.C` on one chain and `old 3` wrapped `II.D` + `II.E`: split into `2|a`–`2|e` in source order; `old 3` joined `II.E` (`2|e`, tie in Card count 3 vs 3 and all 9 Cited backlinks belong to §II.E), trimmed to §II.E with `3|a|2..3` on its Direct chain and finding `3|c` as Side child.
  - `old 5|c|a` (Table VI) was filed under §IV.A because the QJE typeset table floats past the §IV heading, though introduced in the closing paragraph of §III: moved under `3|b|3` as `3|b|3|a` with source line corrected to `§III 表 VI`.
  - `old 5` split into `IV.A` (`4|a`, with model `4|a|a`) and `IV.B` (`4|b`, with model `4|b|a`).
  - `old 6` + `old 7` split into §V opening (`5|a`), whole §V empirical model (`5|b`), `V.A`–`V.D` (`5|c`–`5|f`), and top Card `6` (`六、结论`, promoted from `7|c|5`). `old 7` joined `V.C` (`5|e`, 7 Cards vs 5 in V.D vs 1 in VI, and all 6 Cited backlinks belong to §V.C), trimmed to §V.C.
- **Whole units**: all 4 empirical Model Cards moved intact with 0 parts detached: `4|a` → `3|a` (26 owned parts), `5|a` → `4|a|a` (31 owned parts), `5|b` → `4|b|a` (32 owned + 4 shared = 36 members), `6|a` → `5|b` (113 owned + 4 shared = 117 members).
- **Summary leads & text updates**: `leads.mjs --commit` rewrote `2`, `4`, `5` with `小节索引：`, `在全书中的位置：`, and `来源：`; updated root index `levine2017smart`, structure Card `1|d` (6 sections + 4 surviving models at final addresses), `1`, `3`, `6`, trimmed heads `2|e` and `5|e`, Table VI source line `3|b|3|a`, and prefixed subdivision heads (`2|a`–`2|d`, `4|a`–`4|b`, `5|a`, `5|c`–`5|d`, `5|f`).

### Gate results (post term-relink)

- `zt-digest term-link -Step relink -CitationKey levine2017smart`: **PASS** (`cards_changed: 2, links_added: 8, links_repaired: 0, cards_refused: 0`); final `-PlanOnly`: `cards_changed: 0, links_added: 0, ok: true`.
- `zt-check --tree levine2017smart --snapshot`: **PASS** (`no fault`, exit 0; 374 Cards unchanged, 0 whole-unit size changes).
- `zt-check --cards` (21 edited/lead/root/structure Cards): **PASS** (`faults: 0`, exit 0; fixed 1 initial `R2` line-length fault on `1|d`).
- `model-survey.mjs levine2017smart`: **PASS** — 4 empirical Model Cards (`3|a`, `4|a|a`, `4|b|a`, `5|b`), 4 candidate groups.
- `zt-check --model '<head>'` on all 4 empirical Model Cards (`3|a`, `4|a|a`, `4|b|a`, `5|b`):
  - `--stage final`: **PASS** (exit 0 on all 4; `loose 0`, `self_sufficient true`, `evidence_bound true`)
  - `--settings`: **PASS** (exit 0 on all 4; `settings: data tree and specification ladder are clean`)
  - `--network`: **PASS** (exit 0 on all 4; `network: clean`)
- `list_broken_links` (agent workspace): **PASS** — `[]` (0 broken links total, 0 in `levine2017smart/`).
- No formal models in `levine2017smart` (Stage 7 `zt-model-illustrate` N/A). Post-digest sequence complete.

## Stage 7: illustrate

Not applicable (`levine2017smart` holds 4 empirical models, 0 formal models; Stage 7 applies to formal models only).
