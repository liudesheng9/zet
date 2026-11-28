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
