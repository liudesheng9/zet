# Model Passes: `hurst2011small`

- **Owner:** Xingtong Chen
- **Source:** Hurst & Pugsley (2011), *What Do Small Businesses Do?*, Brookings Papers on Economic Activity (`D:\zt-digest\hurst2011small\book.zh.md`)
- **Live archive:** `D:\zt`
- **Pre-Stage-1 backup:** `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3`

---

## Stage 1: fold

### Decision table

Initial Model Cards surveyed: **6** (`2|a`, `3|a`, `5|a`, `6|a`, `7|a`, `10|a`).
Final surviving models: **5** (4 empirical, 1 concept).

| Initial Model Card | Level | Initial Parts | Model | Surviving Head | Reason |
| --- | --- | ---: | --- | --- | --- |
| `hurst2011small/2|a` | empirical | 28 | M1: 2007 SUSB cross-sectional industry concentration (§I) | `hurst2011small/2|a` | Own empirical model: 2007 SUSB + Economic Census cross-section (~6M employer firms, 294 4-digit NAICS industries) measuring small-business share $x_j$ concentration and within-industry employment share $y_j$ deciles. |
| `hurst2011small/3|a` | empirical | 36 | M2: Ex post growth, innovation & SUSB job-creation panel (§II) | `hurst2011small/3|a` | Own empirical model: distinct multi-dataset micro evidence (BDS, SSBF, KFS, PSED II) and 2003–2006 SUSB annual industry panel estimating ex post growth/innovation shares and job creation/birth/destruction slopes $\gamma_1$ on $x_j$. |
| `hurst2011small/5|a` | empirical | 27 | M3: PSED II startup motives predicting expectations & innovation (§IV) | `hurst2011small/5|a` | Own empirical model: PSED II 2006/2010 nascent-entrepreneur survey with open-ended motive coding (Tables 9–10), estimating conditional mean and P75/P90 quantile differences between nonpecuniary and new-product motive dummies. |
| `hurst2011small/6|a` | concept | 4 | M4: Pugsley (2011a, 2011b) nonpecuniary utility & natural scale conceptual model (§V.A & §VI) | `hurst2011small/6|a` | Surviving head of the conceptual model: heterogeneous household nonpecuniary flow utility (normal good via diminishing marginal utility of consumption) + heterogeneous industry natural returns to scale, with no ability differences or financing constraints. |
| `hurst2011small/7|a` | concept | 4 | M4: Pugsley (2011a, 2011b) nonpecuniary utility & natural scale conceptual model (§V.A & §VI) | `hurst2011small/6|a` | Continues `6|a`: explicitly the same Pugsley (2011a) occupational-choice framework with identical primitives (heterogeneous nonpecuniary flow utility lost on scaling, heterogeneous industry natural returns to scale, normal good, no ability/liquidity friction), adding labor-tax-financed small-business subsidies (no social spillovers) to evaluate efficiency and regressivity in §VI. Split only because digest groups 6 and 7 ran in parallel. |
| `hurst2011small/10|a` | empirical | 25 | M5: Haltiwanger BPEA comment on BDS coworker mean, 6-digit $R^2$ & skewness | `hurst2011small/10|a` | Own empirical model: John Haltiwanger's discussant comment using 2003–2005 BDS full employer microdata (~6M/yr) on distinct estimands (coworker mean $C_j$, 6-digit NAICS $R^2$, age-stratified DHS net growth skewness, startup/high-growth job creation decomposition, and $y_j$ robustness); linked to `3|a` by `对照`. |

### Fold actions

Folded `hurst2011small/7|a` into `hurst2011small/6|a` (all writes serialized via `zt_mutex.py`):

1. Stripped the `**结构**` section from `hurst2011small/7|a` (no `**引入**` section and no remaining links to model parts; no Idea-note Side successors under `7|a`).
2. Parked `hurst2011small/7|a|a` (and its subtree `7|a|a|1`, `7|a|a|1|a`, `7|a|a|2`) as Side successor `hurst2011small/6|a|b` on `hurst2011small/6|a`.
3. Retyped `hurst2011small/7|a` alone into a model part (`facet: "章节：政策一般均衡"`, `emphasis: ""`, `level: ""`).
4. Moved `hurst2011small/7|a` alone onto `6|a`'s main line as `hurst2011small/6|a|a|3` (after `6|a|a|2`), attached its mechanism/example/conclusion subtree under it (`6|a|a|3|a`, `6|a|a|3|a|a`, `6|a|a|3|a|1`), and placed its setting card (`7|a|a`) into `6|a`'s setting part as Side successor `hurst2011small/6|a|a|a`.
5. Ran `zt-digest term-link -Step relink -CitationKey hurst2011small` (0 cards changed; `-PlanOnly` confirmed 0 changes).

### Surviving heads

| Address | Level | Parts | Title |
| --- | --- | ---: | --- |
| `hurst2011small/2|a` | empirical | 28 | 实证测度：基于 SUSB 数据的行业小企业份额与行业内小企业就业份额检验 |
| `hurst2011small/3|a` | empirical | 36 | 实证模型：小企业事后成长、创新与行业小企业份额岗位创造回归（§II） |
| `hurst2011small/5|a` | empirical | 27 | 实证模型：创业动机虚拟变量预测初创企业预期与创新行为（PSED，表 9–10） |
| `hurst2011small/6|a` | concept | 9 | 概念模型：Pugsley（2011a, 2011b）异质性非货币效用与行业自然规模报酬模型 |
| `hurst2011small/10|a` | empirical | 25 | 实证模型：Haltiwanger 对 BDS 同事均值、行业与偏态检验 |

### Stage 1 gate results

- `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs hurst2011small`: **PASS** — 5 Model Cards (`2|a`, `3|a`, `5|a`, `6|a`, `10|a`), 5 candidate groups.
- `zt-check --model '<head>' --stage fold`: **PASS (exit 0 on all 5 heads)**:
  - `hurst2011small/2|a`: 28 members (`main:6, off:1, side:21`), `stage(fold) pass`
  - `hurst2011small/3|a`: 36 members (`main:6, off:1, side:29`), `stage(fold) pass`
  - `hurst2011small/5|a`: 27 members (`main:7, off:1, side:19`), `stage(fold) pass`
  - `hurst2011small/6|a`: 9 members (`main:4, side:5`), `stage(fold) pass` (1 Stage 3 deferred notice: `unanchored` on former head `6|a|a|3`)
  - `hurst2011small/10|a`: 25 members (`main:6, off:1, side:18`), `stage(fold) pass`
- `list_broken_links` for `hurst2011small/`: **0 broken links**.
- Tree card count: **345** (unchanged).
- `zt-digest term-link -Step relink -CitationKey hurst2011small` & `-PlanOnly`: **0 cards changed, 0 pending changes**.
- Note for Stage 3 (`zt-model-restructure` / illustrate): `hurst2011small/6|a|a|3|a` (former `7|a|a|1`) has an inline SVG `data-zt="hurst2011small/7|a|a|2"` attribute that should be refreshed to final addresses in Stage 3/illustrate; `hurst2011small/6|a|a|3` needs a `依赖` rests-on line in Stage 3.

---

## Stage 2: settings

- **Date:** 2026-11-27
- **Pre-Stage-2 backup:** `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`
- **Scope:** All 4 empirical models (`hurst2011small/2|a`, `3|a`, `5|a`, `10|a`); concept model `hurst2011small/6|a` skipped per `zt-post-digest`.
- **Formula & link preservation:** Verified byte-for-byte by script across all 20 edited cards before writing; no cards moved (model head `**结构**` unchanged).

### Per-model changes

1. **`hurst2011small/2|a`** (28 members; 5 cards edited):
   - Added one-line summary + `**起点**` + `**内容**` to data-tree cards `2|a|a|b` (`测量：企业口径与小企业界定`) and `2|a|a|c` (`样本：无雇员企业补充`), keeping existing `依赖 [[hurst2011small/2|a|a]]` lines in `**起点**`.
   - Cleaned redundant `（补：书中未写）` marker in `**估计式**` on `2|a|a|2|a`, `2|a|a|2|b`, `2|a|a|2|c` while preserving the canonical `原书没有写出本规格的估计式。本卡按原文补写：` opening and all display formulas byte-for-byte.
2. **`hurst2011small/3|a`** (36 members; 2 cards edited):
   - Added one-line summary + `**起点**` + `**内容**` to `3|a|a|b` (`测量：DHS 对称岗位变动率`), preserving its display formula byte-for-byte.
   - Cleaned redundant `（补：书中未写）` marker in `**估计式**` on `3|a|a|2|f` (`规格：微观成长与创新描述比例`).
3. **`hurst2011small/5|a`** (27 members; 7 cards edited):
   - Added one-line summary + `**起点**` + `**内容**` to `5|a|a|b` (`测量：五大创业动机编码与表 A2`) and `5|a|a|c` (`样本：2006 正收入与 2010 存续样本`).
   - Cleaned redundant `（补：书中未写）` marker in `**估计式**` on `5|a|a|2`, `5|a|a|2|a`, `5|a|a|2|b`, `5|a|a|2|c`, `5|a|a|2|d`.
4. **`hurst2011small/10|a`** (25 members; 6 cards edited):
   - Added one-line summary + `**起点**` + `**内容**` to `10|a|a|b` (`测量：同事均值与高成长界定`).
   - Cleaned redundant `（补：书中未写）` marker in `**估计式**` on `10|a|a|2`, `10|a|a|2|a`, `10|a|a|2|b`, `10|a|a|2|c`, `10|a|a|2|d`.

### Stage 2 gate results (post term-relink)

- `zt-digest term-link -Step relink -CitationKey hurst2011small`: `cards_changed: 3, links_added: 6, links_repaired: 0, cards_refused: 0`; `-PlanOnly` confirmed `cards_changed: 0, links_added: 0`.
- `zt-check --model '<head>' --settings`: **PASS (exit 0 on all 4 empirical models)**:
  - `hurst2011small/2|a`: `settings: data tree and specification ladder are clean`
  - `hurst2011small/3|a`: `settings: data tree and specification ladder are clean`
  - `hurst2011small/5|a`: `settings: data tree and specification ladder are clean`
  - `hurst2011small/10|a`: `settings: data tree and specification ladder are clean`
- `zt-check --model '<head>' --stage settings`: **PASS (exit 0 on all 4 empirical models)**:
  - `hurst2011small/2|a`: 28 members (`main:6, off:1, side:21`), `self_sufficient true; evidence_bound true; stage(settings) pass`
  - `hurst2011small/3|a`: 36 members (`main:6, off:1, side:29`), `self_sufficient true; evidence_bound true; stage(settings) pass`
  - `hurst2011small/5|a`: 27 members (`main:7, off:1, side:19`), `self_sufficient true; evidence_bound true; stage(settings) pass`
  - `hurst2011small/10|a`: 25 members (`main:6, off:1, side:18`), `self_sufficient true; evidence_bound true; stage(settings) pass`
- `node E:\zet\skills\zt-model-settings\scripts\check-settings.mjs '<head>' --symbols`: **PASS (exit 0 on all 4 empirical models)** — `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` for `hurst2011small/`: **0 broken links**.

---

## Stage 3: restructure

- **Date:** 2026-11-27
- **Pre-Stage-3 backup:** `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`
- **Scope:** All 4 empirical models (`hurst2011small/2|a`, `3|a`, `5|a`, `10|a`); concept model `hurst2011small/6|a` skipped per `zt-post-digest`.
- **Formula, estimate & link preservation:** Verified by script before writing across all 16 edited cards (all display formulas `$$...$$`, empirical estimates, and existing links preserved byte-for-byte; no cards moved, so head `**结构**` sections stay unchanged).

### Per-model changes

1. **`hurst2011small/2|a`** (28 members; 4 cards edited; 3 chapters, no `**分部**` needed):
   - Added crisp `**直觉**` leads (>=20 chars) to main-line cards `2|a|a` (`数据`), `2|a|a|1` (`设计`), and `2|a|a|2` (`规格：基准`).
   - Added plain reference line `见设计 [[hurst2011small/2|a|a|1]]。` to off-spine limits card `2|a|b` (`loose` 1 → 0).
2. **`hurst2011small/3|a`** (36 members; 4 cards edited; 3 chapters, no `**分部**` needed):
   - Added crisp `**直觉**` leads (>=20 chars) to main-line cards `3|a|a` (`数据`), `3|a|a|1` (`设计`), and `3|a|a|2` (`规格：基准`).
   - Fixed `R22` bare LaTeX in `3|a|a|2` (`g_{jt}^{Ms}` → `$g_{jt}^{Ms}$`).
   - Added plain reference line `见设计 [[hurst2011small/3|a|a|1]]。` (and replaced `本模型` with `该实证分析`) on off-spine limits card `3|a|b` (`loose` 1 → 0).
3. **`hurst2011small/5|a`** (27 members; 4 cards edited; 4 chapters, no `**分部**` needed):
   - Added crisp `**直觉**` leads (>=20 chars) to main-line cards `5|a|a` (`数据`), `5|a|a|1` (`设计`), and `5|a|a|2` (`规格：基准`).
   - Fixed 2 `R22` bare LaTeX faults in `5|a|a|2` (`D_{idea,i}` → `$D_{\text{idea},i}$`, `D_{nonpec,i}` → `$D_{\text{nonpec},i}$`).
   - Added plain reference line `见设计 [[hurst2011small/5|a|a|1]]。` (and replaced `本实证模型` with `该实证分析`) on off-spine limits card `5|a|b` (`loose` 1 → 0).
4. **`hurst2011small/10|a`** (25 members; 4 cards edited; 3 chapters, no `**分部**` needed):
   - Added crisp `**直觉**` leads (>=20 chars) to main-line cards `10|a|a` (`数据`), `10|a|a|1` (`设计`), and `10|a|a|2` (`规格：基准`).
   - Added plain reference line `见设计 [[hurst2011small/10|a|a|1]]。` to off-spine limits card `10|a|b` (`loose` 1 → 0).

### Stage 3 gate results (post term-relink)

| Model | members (main/side/off) | loose | `--stage restructure` | `--network` | `--settings` | `--skip-shared` | `--premises` |
| --- | --- | ---: | --- | --- | --- | --- | --- |
| `hurst2011small/2|a` | 28 (6/21/1) | 0 | pass (rc=0) | clean (7 blocks, 5 layers; rc=0) | clean (rc=0) | faults: 0 (3 adv R20; rc=0) | none (rc=0) |
| `hurst2011small/3|a` | 36 (6/29/1) | 0 | pass (rc=0) | clean (7 blocks, 4 layers; rc=0) | clean (rc=0) | faults: 0 (3 adv R20; rc=0) | 4 SE/pointer hits (0 hidden uses) |
| `hurst2011small/5|a` | 27 (7/19/1) | 0 | pass (rc=0) | clean (8 blocks, 5 layers; rc=0) | clean (rc=0) | faults: 0 (4 adv R20; rc=0) | none (rc=0) |
| `hurst2011small/10|a` | 25 (6/18/1) | 0 | pass (rc=0) | clean (7 blocks, 4 layers; rc=0) | clean (rc=0) | faults: 0 (3 adv R20; rc=0) | none (rc=0) |

- **`--premises` hits on `hurst2011small/3|a` (all 4 documented, 0 hidden forward uses):**
  - `3|a|a|4` (`第 1 章` `**本章估计**` pointer list) and `3|a|a|4|a` (`表 5` regression table) contain standard errors `(0.25)` and `(0.27)` next to coefficients `-2.14` and `-0.73`. Because estimate cards `3|a|a|4|b` and `3|a|a|4|c` carry `-2.14 (0.25)` and `-0.73 (0.27)` in their titles, `check-premises` matches `(0.25)` and `(0.27)` as equation numbers. All 4 are standard-error/chapter-pointer references and stay untouched.
- **Term relink:** `zt-digest term-link -Step relink -CitationKey hurst2011small` applied `cards_changed: 4, links_added: 5, links_repaired: 0, cards_refused: 0, rounds: 1`; `-PlanOnly` confirmed `cards_changed: 0, links_added: 0`.
- **Broken links:** `list_broken_links` for `hurst2011small/` (and full agent workspace): `[]` (0 broken links).
- **Stage 3 invariants:** `self_sufficient: true`, `evidence_bound: true` on all 4 models; no cards moved or deleted.

---

## Stage 4: imports and explanations

- **Date:** 2026-11-27
- **Pre-Stage-4 backup:** `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3`
- **Scope:** All 4 empirical models (`hurst2011small/2|a`, `3|a`, `5|a`, `10|a`); concept model `hurst2011small/6|a` inspected solely as a potential lender of theoretical predictions.
- **Writes to `D:\zt`:** 0 card edits; `term-link -Step relink` serialized via `zt_mutex.py` (0 cards changed).

### Import decisions

1. **Theoretical predictions from `hurst2011small/6|a` (concept model):** None imported. None of `2|a`, `3|a`, `5|a`, or `10|a` contains a `假说` card testing a prediction of `6|a`; in the paper, Pugsley's (2011a, 2011b) conceptual model (§V.A & §VI) is introduced *after* the §I/§II/§IV empirical facts to interpret them rather than as a formal theory whose predictions are tested.
2. **Same-book empirical data/measure blocks (`2|a`, `3|a`, `5|a`, `10|a`):** None imported; each empirical model owns a distinct data/measurement design with 0 outside premises, 0 legacy shares, 0 false shares, 0 copy advices, and 0 tolerated premises (`imports.mjs check --json` and `imports.mjs plan --json` both returned `faults: [], changes: [], manual: [], advice: []` on all 4 heads):
   - **`2|a` → `3|a`:** `2|a` uses the 2007 SUSB + Economic Census cross-section (`2|a|a`, `2|a|a|b`, `2|a|a|c`) for decile concentration, whereas `3|a` combines five datasets (2005 BDS, 2003 SSBF, 2004–2008 KFS, 2006–2010 PSED II, and the 2003–2006 SUSB annual panel) with DHS symmetric job-flow measurement (`3|a|a|b`). Importing `2|a|a|b` or `2|a|a|2` `整块` would pull the 2007 cross-section data (`2|a|a`) or decile-sorting design (`2|a|a|1`) into `3|a`; the shared concept $x_j$ is already linked via Term Card `[[行业小企业份额@hurst2011small/1|b|h]]`.
   - **`3|a` → `5|a`:** `5|a` uses PSED II 2006/2010 (1,214 / 602 / 162) with its own 44→5 open-ended motive coding (`5|a|a|b`) and 3-subsample design (`5|a|a|c`); importing `3|a|a` would pull BDS/SSBF/KFS/SUSB into a pure PSED II model.
   - **`2|a`/`3|a`/`5|a` → `10|a`:** `10|a` is John Haltiwanger's BPEA discussant comment using 2003–2005 BDS full employer microdata (~6M/yr) and distinct estimands (coworker mean $C_j$, 6-digit NAICS $R^2$, firm-level DHS net growth skewness); its links with `3|a` and `5|a` are `对照` comparisons and remain comparisons, not premises.
3. **Cross-book blocks:** None (`0` external-book links across all 4 empirical models).

### Explanation refresh (step 8)

- `zt-digest model-explain -Step plan` returned `nothing to explain` (`parts: 0, jobs: []`) on all 4 empirical models (`2|a`, `3|a`, `5|a`, `10|a`). None of the four empirical models has `推导：估计式` parts, and no estimate premises changed.

### Stage 4 final gate results (post term-relink)

| Model | members (main/side/off) | `imports check` | `--stage final` | `--settings` | `--network` | `--skip-shared` |
| --- | --- | --- | --- | --- | --- | --- |
| `hurst2011small/2|a` | 28 (6/21/1) | ok, 0 faults, 0 advice (rc=0) | pass (`self_sufficient true`, `evidence_bound true`; rc=0) | clean (rc=0) | clean (7 blocks, 5 layers; rc=0) | faults: 0 (3 adv R20; rc=0) |
| `hurst2011small/3|a` | 36 (6/29/1) | ok, 0 faults, 0 advice (rc=0) | pass (`self_sufficient true`, `evidence_bound true`; rc=0) | clean (rc=0) | clean (7 blocks, 4 layers; rc=0) | faults: 0 (3 adv R20; rc=0) |
| `hurst2011small/5|a` | 27 (7/19/1) | ok, 0 faults, 0 advice (rc=0) | pass (`self_sufficient true`, `evidence_bound true`; rc=0) | clean (rc=0) | clean (8 blocks, 5 layers; rc=0) | faults: 0 (4 adv R20; rc=0) |
| `hurst2011small/10|a` | 25 (6/18/1) | ok, 0 faults, 0 advice (rc=0) | pass (`self_sufficient true`, `evidence_bound true`; rc=0) | clean (rc=0) | clean (7 blocks, 4 layers; rc=0) | faults: 0 (3 adv R20; rc=0) |

- **Term relink:** `zt-digest term-link -Step relink -CitationKey hurst2011small` (under `zt_mutex.py`): `cards_changed: 0, links_added: 0, links_repaired: 0, cards_refused: 0`; `-PlanOnly` confirmed `cards_changed: 0, links_added: 0`.
- **Broken links:** `list_broken_links` for `hurst2011small/` (and full agent workspace): `[]` (0 broken links).


## Stage 5: evidence dialogue

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; judged by Gemini 4 Neon Medium swarm workers, 0 DeepSeek cost). Backup: `D:\zt\backups\zt-before-iof10-stage5-dialogue-2026-11-27.sqlite3`.

| Model | Unique pairs judged | Same estimand (印证 / 反驳 / 对照) | Cards written | Check |
|---|---|---|---|---|
| `2|a` | 80 | 0 (0 / 0 / 0) | 0 | `{"head":"hurst2011small/2|a","broken":0,"proposed_again":0,"ok":true}` |
| `3|a` | 34 | 1 (0 / 0 / 1) | 2 | `{"head":"hurst2011small/3|a","broken":0,"proposed_again":0,"ok":true}` |
| `5|a` | 5 | 0 (0 / 0 / 0) | 0 | `{"head":"hurst2011small/5|a","broken":0,"proposed_again":0,"ok":true}` |
| `10|a` | 32 | 0 (0 / 0 / 0) | 0 | `{"head":"hurst2011small/10|a","broken":0,"proposed_again":0,"ok":true}` |

- Term relink: `{"ok":true,"complete":true,"book":"hurst2011small","cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"rounds":0,"applied":false}`; `-PlanOnly`: `{"ok":true,"complete":true,"book":"hurst2011small","cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"rounds":0,"applied":false}`.

---

## Stage 6: literature organize

- **Date:** 2026-11-27
- **Pre-Stage-6 backup:** `D:\zt\backups\zt-before-iof10-stage6-organize-2026-11-27.sqlite3`
- **Live archive:** `D:\zt` (all writes serialized via `zt_mutex.py`; 0 DeepSeek calls; tree card count unchanged at **345**)

### Source divisions

From `D:\zt-digest\hurst2011small\book.zh.md`:
- 引言（`# 小企业究竟做什么？`，含摘要与引言）
- I. 小企业的行业构成
- II. 小企业的事后成长与创新（`II.A. 小企业的成长`、`II.B. 小企业的创新`）
- III. 关于成长与创新的事前预期
- IV. 创业动机
- V. 创业动机或预期的异质性为何重要（`V.A. 企业动态`、`V.B. 理解风险—收益权衡`、`V.C. 发展中经济体中的小企业`）
- VI. 政策含义
- VII. 结论
- 附录（表 A1、表 A2 与致谢）
- 评论与讨论（`John Haltiwanger 评论`、`Haltiwanger 评论参考文献`、`Adam Looney 评论`、`Looney 评论参考文献`、`总体讨论`）
- Tail lists: 术语表（48 个术语）、参考文献（41 篇文献）

### Final top line (12 top Cards, 345 total)

```
 1  引言：小企业主与企业家模型的差异及全文结构 (12)              1|c..1|c|7 (含 1|c|7|a 引言脚注 2、3) · 1|d 结构卡
 2  I. 小企业的行业构成 (44)                                  2|a 实证模型 M1 (28 parts) · 2|b..2|b|7 · 2|c..2|f Findings
 3  II. 小企业的事后成长与创新 (54, rewrite)                   3|a II.A (含 3|a|a 实证模型 M2 [36 parts] + 5 Findings) · 3|b II.B
 4  III. 关于成长与创新的事前预期 (9)                          4|a..4|a|3 · 4|b..4|e Findings
 5  IV. 创业动机 (41)                                         5|a 实证模型 M3 (27 parts) · 5|b..5|b|2 · 5|c..5|h Findings
 6  V. 创业动机或预期的异质性为何重要 (18, rewrite)             6|a V.A (含 6|a|a 概念模型 M4 [9 parts]) · 6|b V.B · 6|c V.C
 7  VI. 政策含义 (6)                                          7|b..7|b|4
 8  VII. 结论 (5)                                             8|a..8|a|3
 9  附录 (6)                                                  9|a..9|a|2 · 9|b..9|c Findings
10  评论与讨论 (58, rewrite)                                  10|a Haltiwanger 评论 (含 10|a|a 实证模型 M5 [25 parts] + 6 Findings)
                                                              10|b Haltiwanger 评论参考文献 · 10|c Adam Looney 评论 (含 4 Events)
                                                              10|d Looney 评论参考文献 · 10|e 总体讨论 (含 1 Event + 2 Findings)
11  术语表 (49, from 1|b)
12  参考文献 (42, from 1|a)
```

### Crossers settled and edits

- **Batches (tail-to-head, 4 atomic `batch.mjs --commit` runs, 0 breaks, 0 refusals):**
  1. `01_comments.ops` (57 renamed, 361 rewrites): merged old `11` into `10` (`评论与讨论`); detached `11|a|1..6` (`总体讨论`) from `11|a` (`Looney 评论参考文献`), placed `11` as `10|e` (`总体讨论`), split `10|b|5` (`Haltiwanger 评论参考文献`) and `10|b|6..8` (`Adam Looney 评论`), moved Model M5 (`10|a`) and 6 Haltiwanger Findings (`10|c..h`) under `10|a`, and reordered `10` into the 5 source `##` subdivisions (`10|a`–`10|e`).
  2. `02_sec5.ops` (17 renamed, 60 rewrites): split `6|b` chain into `V.A` (`6|a`), `V.B` (`6|b`), `V.C` (`6|c`), and moved Concept Model M4 (`6|a`) intact under `V.A` (`6|a|a`).
  3. `03_sec2.ops` (53 renamed, 373 rewrites): split `3|b` chain into `II.A` (`3|a`) and `II.B` (`3|b`), and moved Empirical Model M2 (`3|a`) and 5 `§II.A` Findings (`3|c..g`) under `II.A` (`3|a|a`–`3|a|f`).
  4. `04_intro_lists.ops` (92 renamed, 776 rewrites): moved OCR-displaced Introduction footnote Card `2|b|a` (`引言脚注 2 与 3`) to `1|c|7|a`, and moved Term list (`1|b`) then Cited list (`1|a`) to tail top Cards `11` and `12`.
- **Summary leads (`leads.mjs --commit`):** rewrote `3` (`§II`), `6` (`§V`), and `10` (`评论与讨论`).
- **Head/root/structure edits (14 `edit_card` calls):** dropped `第 N 组：` prefixes and added `来源：` lines on `1`, `2`, `4`, `5`, `7`, `8`, `9`; updated `1|c|7|a` source line (`引言脚注 2、3`), prefixed `3|a` (`II.A.`) and `3|b` (`II.B.`), retitled/trimmed `10|e` to `总体讨论`, repointed `10|d` link to `10|c`, and updated root index (`hurst2011small`) and structure Card (`1|d`) to final addresses and 5 surviving models.

### Final Model Card addresses (`model-survey.mjs`)

| Final Address | Level | Parts | Model |
| --- | --- | ---: | --- |
| `hurst2011small/2|a` | empirical | 28 | M1: 2007 SUSB cross-sectional industry concentration (§I) |
| `hurst2011small/3|a|a` | empirical | 36 | M2: Ex post growth, innovation & SUSB job-creation panel (§II) |
| `hurst2011small/5|a` | empirical | 27 | M3: PSED II startup motives predicting expectations & innovation (§IV) |
| `hurst2011small/6|a|a` | concept | 9 | M4: Pugsley (2011a, 2011b) nonpecuniary utility & natural scale conceptual model (§V.A & §VI) |
| `hurst2011small/10|a|a` | empirical | 25 | M5: Haltiwanger BPEA comment on BDS coworker mean, 6-digit $R^2$ & skewness |

### Stage 6 gate results (post term-relink)

- **Term relink:** `zt-digest term-link -Step relink -CitationKey hurst2011small` and `-PlanOnly`: `ok: true, cards_changed: 0, links_added: 0, links_repaired: 0, cards_refused: 0`.
- **`zt-check --cards` (17 edited/lead/root/structure Cards):** `faults: 0` (RC=0).
- **`zt-check --tree hurst2011small --snapshot` & `check.mjs`:** `no fault` (RC=0; 12 top Cards, 345 total, 0 model splits).
- **`model-survey.mjs hurst2011small`:** 5 Model Cards (`2|a`, `3|a|a`, `5|a`, `6|a|a`, `10|a|a`; RC=0).
- **Empirical model checks at final addresses (`2|a`, `3|a|a`, `5|a`, `10|a|a`):** `--stage final` (`self_sufficient true`, `evidence_bound true`, `pass`), `--settings` (`clean`), and `--network` (`clean`) all exit **0**.
- **`list_broken_links` (`hurst2011small/` & full agent workspace):** `{"total_broken":0,"hurst_broken":[]}`.

## Stage 7: illustrate

Not applicable (`hurst2011small` holds 4 empirical models and 1 concept model, 0 formal models; Stage 7 applies to formal models only).
