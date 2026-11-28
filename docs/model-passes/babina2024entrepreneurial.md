# babina2024entrepreneurial: post-digest model passes

Owner: Xingtong Chen

Babina and Howell (2024), *Entrepreneurial Spillovers from Corporate R&D* (Journal of Financial Economics). Post-digest sequence (`zt-post-digest`) on the live box `D:\zt`.

## Stage 1: fold

Date: 2026-11-27, live box (`D:\zt`). Pre-Stage-1 backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3`.

Survey before: 5 Model Cards (1 concept, 4 empirical), 5 candidate groups (all singleton groups; `5|a` and `6|a` linked by `kin`/`compare` read lines, and `5|a`/`6|a` linked to `4|a` and `3|a` by `kin` read lines). Total tree count: 342 Cards.

### Decision table

| Model Card | Section | Level | Model | Surviving head | Reason |
|---|---|---|---|---|---|
| `3\|a` | §II | concept | Conceptual model: contractual frictions & diversification cost | `3\|a` | Own conceptual model (4 parts): parent R&D, unverifiable effort, firm's 3 options (sell, JV/share, allow spinout), residual control rights, and diversification cost; theory is never folded with empirical models. |
| `4\|a` | §III (Table 1) | empirical | Shared 5-source data, measurement & 3-level descriptive baseline | `4\|a` | Own measurement & descriptive model (25 parts): constructs the 5-source matched panel (Compustat/LBD/31-state LEHD/VentureXpert/NBER patents), top-5 earner employee-entrepreneurship definition, quasi-median rule, representativeness/attrition checks, and Table 1 moments across all three levels (10,500 firm-years, 36,000 establishment-years, 108,000 startups); shared upstream measurement & descriptive baseline to be imported by both `6\|a` and `7\|a` in Stage 4 (also preserves cross-book Model Card links from `azoulay2020age/2\|a` and `stuart2006scientists/4\|a`). |
| `5\|a` | §IV.A–IV.B | empirical | Establishment-panel R&D → employee entrepreneurship (OLS, heterogeneity & IV) | `6\|a` | Same empirical model as `6\|a` (14 parts): specifies the exact same Eq. (1) firm-FE OLS and Eq. (2) R&D tax-price IV on the exact same 31-state establishment-year panel ($N=36,000$) and estimand ($\beta$ on $\ln(\mathrm{R\&D}_{f,t-1})$), including the same baseline OLS ($0.109^*$) and first-stage IV ($-1.960^{***}, F=24.8$) estimates; folded into `6\|a`. |
| `6\|a` | §IV.C, §V (Tables 2–4) | empirical | Establishment-panel R&D → employee entrepreneurship (OLS, heterogeneity & IV) | `6\|a` | Surviving head of the establishment-panel causal model (41 parts before fold, 56 after folding `5\|a` and its 14 parts): holds the full §V OLS (Table 2), parent-firm heterogeneity (Table 3), 2SLS IV (Table 4), and robustness/placebo results (Tables A.1, A.7–A.12), and preserves cross-book Model Card links from `franco2006spinouts/5\|a` and `stuart2006scientists/5\|a`. |
| `7\|a` | §VI (Table 5, Table A.14) | empirical | Startup-level mechanisms & supply-chain linkages | `7\|a` | Own empirical model (25 parts): distinct observation unit ($N=108,000$ employee-founded startups + BEA IO tables + Gompers et al. 2005 out-of-sample VC data), cross-sectional design without firm FE, and distinct estimands (VC backing, incorporation, high-tech, wages, 5-year exit, same-SIC2, upstream/downstream IO proximity, patent citation flows, internalization rate). |

Four surviving models.

### Fold actions

Executed on live box `D:\zt` under `zt_mutex.py` (1 fold: `5|a` -> `6|a`):

1. Stripped `**结构**` from `babina2024entrepreneurial/5|a` (no `**引入**` section present; no Idea-note Direct/Side successors hanging off `5|a`) and masked its 2 remaining model-part links in `**主要估计**` (`5|a|a|4|a`, `5|a|a|3`).
2. Parked `5|a`'s 14 parts (`5|a|a` subtree and `5|a|b`) as Side successors of `6|a` (`6|a|c` and `6|a|d`) in an atomic `apply_batch` (`tidy`).
3. Retyped `5|a` alone via `edit_card` (`facet: "第 4 章：基准 OLS 与税价 IV 设定（§IV.A–B）"`, `emphasis: "core"`, `level: ""`).
4. Moved `5|a` `alone` to the end of `6|a`'s main line (`after: babina2024entrepreneurial/6|a|a|6` -> `babina2024entrepreneurial/6|a|a|7`) and moved the parked parts back under it (`6|a|a|7|a` subtree and `6|a|a|7|b`) in one atomic `tidy` batch.
5. Restored the 2 masked links on `babina2024entrepreneurial/6|a|a|7` using the composed rename map (`5|a|a|4|a` -> `6|a|a|7|a|4|a`; `5|a|a|3` -> `6|a|a|7|a|3`). No mathematics or non-`**结构**` text was altered and no Links were deleted.

| Former head | Now | Facet |
|---|---|---|
| `5\|a` | `6\|a\|a\|7` | `第 4 章：基准 OLS 与税价 IV 设定（§IV.A–B）` |

### Surviving heads

| Head | Level | Parts | Summary |
|---|---|---|---|
| `babina2024entrepreneurial/3\|a` | concept | 4 | Conceptual model: contractual frictions, unverifiable effort, residual control rights & diversification cost (§II) |
| `babina2024entrepreneurial/4\|a` | empirical | 25 | Shared 5-source data, employee-entrepreneurship measurement & Table 1 three-level descriptive baseline (§III) |
| `babina2024entrepreneurial/6\|a` | empirical | 56 | Establishment-panel R&D → employee entrepreneurship: OLS, heterogeneity, R&D tax-price IV & robustness (§IV–§V, Tables 2–4) |
| `babina2024entrepreneurial/7\|a` | empirical | 25 | Startup-level mechanisms, BEA supply-chain linkages & alternative-mechanism tests (§VI, Table 5, Table A.14) |

### Gate results

- `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs babina2024entrepreneurial`: PASS — 4 Model Cards, 4 candidate groups, matching the 4 surviving heads above.
- `zt-check --model '<head>' --stage fold`: PASS (exit 0 on all 4 surviving heads):
  - `3|a`: `members 4 {"main":4}`; `loose 0`; `self_sufficient true`; `stage(fold) pass`.
  - `4|a`: `members 25 {"main":4,"off":2,"side":19}`; `loose 2`; `self_sufficient true`; `evidence_bound true`; `stage(fold) pass`.
  - `6|a`: `members 56 {"main":8,"off":1,"side":47}`; `loose 2`; `stage(fold) pass` (2 expected deferred notices: `[deferred -> Stage 3 (restructure)] unanchored` on former head `6|a|a|7`, and `[deferred -> Stage 2 (settings)] spec_unbound` on `6|a|a|7|a|3`).
  - `7|a`: `members 25 {"main":6,"off":1,"side":18}`; `loose 1`; `self_sufficient true`; `evidence_bound true`; `stage(fold) pass`.
- `list_broken_links` (`agent` workspace): 0 broken links total, 0 touching `babina2024entrepreneurial/`; tree card count unchanged at 342.
- `zt-digest term-link -Step relink -CitationKey babina2024entrepreneurial` (under `zt_mutex.py`) and `-PlanOnly`: `{"ok":true,"complete":true,"book":"babina2024entrepreneurial","cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"rounds":0,"applied":false}`.

## Stage 2: settings

Date: 2026-11-27, live box (`D:\zt`). Pre-Stage-2 backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`. All writes executed under `zt_mutex.py`; concept model `3|a` skipped per `zt-post-digest`.

### `4|a` (empirical, 25 parts)

- `4|a|a|a` (`变量表`): dropped unused `$W_{e,t}$` row (`$W$` appeared only inside `\sum_{w \in W_{e,t}}` subscripts in `4|a|a|2`).
- `4|a|a|b` (`测量：员工创业与分支单元`) and `4|a|a|c` (`测量：准中位数与舍入`): added one-line summary, `**起点**` (`- 依赖 [[babina2024entrepreneurial/4|a|a]]：…`), and `**内容**`, keeping all existing content and `**直觉**` intact. No structural moves.

### `6|a` (empirical, 56 -> 50 parts)

Dissolved folded chapter `6|a|a|7`'s duplicate `数据`/`变量表`/`假说`/`设计`/`规格` sub-hierarchy in one atomic `apply_batch` (12 items, 23 link macros rewritten, 0 broken links):

- Moved surviving unique parts out first:
  - `6|a|a|7|a|2|a` -> `6|a|a|2|d` (renumbered to `假设 4：企业固定效应与多维时间交互 FE 吸收选择与区域冲击`)
  - `6|a|a|7|a|2|b` -> `6|a|a|2|e` (renumbered to `假设 5：时变基本面、员工结构、专利与市场研发不混杂效应`)
  - `6|a|a|7|a|3|a` -> `6|a|a|3|l` (`规格：研发税收价格 IV`, preserving original §IV.B equation `\tag{2}`)
  - `6|a|a|7|a|3|b` -> `6|a|a|3|m` (`规格：稳健性拓展`, preserving its equation)
  - `6|a|a|7|a|4` (+ `4|a`, `4|b`) -> `6|a|a|7|a` (+ `a|a`, `a|b`), reordered ahead of `局限` at `6|a|a|7|b`.
- Deleted 6 redundant duplicates `into` their surviving unified counterparts (leaves first, re-pointing all links):
  - `6|a|a|7|a|a` (`变量表`) `into` `6|a|a|a` (added `$W_{e,f,t}$` and `$\gamma$` rows and noted §IV.B eq. (2) timing on `$\mathrm{R\&D}_{f,t-1}$`, `$\rho_{f,t-1}^{S}$`, `$\rho_{f,t-1}^{F}$`).
  - `6|a|a|7|a|2|c` (`假设 3：排他性约束`) `into` `6|a|a|2|b` (`假设 2`, merged unique FE remark).
  - `6|a|a|7|a|3` (`规格：基准`, byte-for-byte duplicate of eq. `(1)`) `into` `6|a|a|3`.
  - `6|a|a|7|a|2` (`设计`) `into` `6|a|a|2` (merged §IV.A–B variation/comparison/threat details and listed `假设 4`–`5`).
  - `6|a|a|7|a|1` (`假说：研发促进创业`) `into` `6|a|a|1`.
  - `6|a|a|7|a` (`数据`) `into` `6|a|a` (merged flow-R&D, control-set, and tax-price IV details).
- Rebuilt `**结构**` on `babina2024entrepreneurial/6|a` from live `build.order` (all 50 members).

### `7|a` (empirical, 25 parts)

Already clean in baseline; unchanged and re-verified after term relink.

### Gate results (post term-relink)

- Term relink (`zt-digest term-link -Step relink -CitationKey babina2024entrepreneurial` under `zt_mutex.py` and `-PlanOnly`): `{"ok":true,"complete":true,"book":"babina2024entrepreneurial","cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"rounds":0,"applied":false}`.
- `zt-check --model '<head>' --settings` (exit 0 on all 3 empirical models):
  - `4|a`: `settings: data tree and specification ladder are clean`
  - `6|a`: `settings: data tree and specification ladder are clean`
  - `7|a`: `settings: data tree and specification ladder are clean`
- `zt-check --model '<head>' --stage settings` (exit 0 on all 3 empirical models):
  - `4|a`: `members 25 {"main":4,"off":2,"side":19}`; `relations {"refers":34,"rests-on":12}`; `loose 2`; `self_sufficient true`; `evidence_bound true`; `stage(settings) pass`
  - `6|a`: `members 50 {"main":8,"off":1,"side":41}`; `relations {"corresponds":1,"illustrates":5,"refers":83,"rests-on":19}`; `loose 2`; `evidence_bound true`; `stage(settings) pass` (1 expected `[deferred -> Stage 3 (restructure)] unanchored` on folded chapter `6|a|a|7`)
  - `7|a`: `members 25 {"main":6,"off":1,"side":18}`; `relations {"corresponds":1,"illustrates":5,"refers":40,"rests-on":13}`; `loose 1`; `self_sufficient true`; `evidence_bound true`; `stage(settings) pass`
- `list_broken_links` (`agent` workspace): 0 broken links total, 0 for `babina2024entrepreneurial/`.
- Formula preservation script: all 25 unique display formulas across `4|a`, `6|a`, `7|a` preserved byte-for-byte (0 missing).
- Tree card count: 342 -> 336 (6 duplicate parts merged `into` surviving counterparts in `6|a`).

## Stage 3: restructure

Date: 2026-11-27, live box (`D:\zt`). Pre-Stage-3 backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`. All writes executed under `zt_mutex.py`; concept model `3|a` skipped per `zt-post-digest`.

### `4|a` (empirical, 25 parts)

- Added `**直觉**` leads (>= 20 chars) to `4|a|a|1` (`设计`) and `4|a|a|2` (`规格：基准`), clearing both `--network` `lead` faults.
- Anchored both off-spine limitation cards (`4|a|b`, `4|a|c`) with plain `见设计 [[babina2024entrepreneurial/4|a|a|1]]。` links (`loose` 2 -> 0).
- No structural moves; all 2 display formulas and all empirical estimates unchanged.

### `6|a` (empirical, 50 -> 49 parts)

- **Structural `tidy` batch** (1 atomic `apply_batch`, 37 link macros rewritten, 0 broken links):
  - Dissolved redundant inner chapter wrapper `6|a|a|7|a` (`第 1 章：基准与一阶检验`) `into` `6|a|a|7`: promoted `6|a|a|7|a|a` (`估计：式 (1) 基准 OLS`) and `6|a|a|7|a|b` (`估计：式 (2) IV 第一阶段`) to Side branches `6|a|a|7|a` and `6|a|a|7|b`, ordered ahead of `局限：时间下标与附录缺失` at `6|a|a|7|c`.
  - Reordered `6|a|a|3`'s Side branches so `规格：研发税收价格 IV` (which defines eq. `(2)`, formerly `3|l`) sits at `6|a|a|3|j` ahead of `表 4 偏好 IV` (`3|k`) and `表 4 含控制 IV` (`3|k|a`), eliminating both true forward `--premises` hits on `(2)`.
- **Card & head edits** (18 cards):
  - Rewrote folded chapter `6|a|a|7` into clean chapter format (`第 4 章：基准 OLS 方程 (1) 与研发税收价格一阶方程 (2) 对应估计（§IV.A–B）`, anchored with `依赖 [[babina2024entrepreneurial/6|a|a|3]]`, `**直觉**`, `**本章估计**` listing `7|a` and `7|b`, verbatim `**对应方程**` preserving both display formulas `(1)` and `(2)`, and `**承上启下**`), clearing `unanchored`, `lead`, `R13`, and `R22`. Model has 4 chapters (`第 1–4 章`), so no `**分部**` section needed.
  - Added `**直觉**` leads (>= 20 chars) to `6|a|a` (`数据`), `6|a|a|1` (`假说`), `6|a|a|2` (`设计`), `6|a|a|3` (`规格：基准`), and `6|a|a|7`.
  - Cleared all 20 blocking `--skip-shared` faults: fixed `R22` bare LaTeX on `6|a`, `6|a|a|3`, `6|a|a|3|a`–`h`, `6|a|a|3|j` (formerly `3|l`), `6|a|a|4|e`, and `6|a|a|7`; fixed `R2`/`R3` semicolon/length splits on `6|a|a` (items 6–8) and `6|a|a|2` (`工具变量 2SLS`).
  - Anchored both loose limitation cards (`6|a|a|7|c` -> `见 [[babina2024entrepreneurial/6|a|a|7]]。`; `6|a|b` -> `见设计 [[babina2024entrepreneurial/6|a|a|2]]。`), clearing `loose` 2 -> 0.
  - Rebuilt `**结构**` on `babina2024entrepreneurial/6|a` from live `build.order` (all 49 members).

### `7|a` (empirical, 25 parts)

- Added `**直觉**` leads (>= 20 chars) to `7|a|a|1` (`假说`), `7|a|a|2` (`设计`), and `7|a|a|3` (`规格：基准`).
- Fixed 1 `R22` bare LaTeX fault on `7|a|a|3` (`$\text{Firm log }\mathrm{R\&D}_{t-1}$`).
- Anchored off-spine limitation card `7|a|b` with `见设计 [[babina2024entrepreneurial/7|a|a|2]]。` (`loose` 1 -> 0).
- No structural moves; all 5 display formulas and all empirical estimates unchanged.

### Gate results (post term-relink)

| Model | Members (main/side/off) | Loose | `--stage restructure` | `--network` | `--settings` | `--skip-shared` | `--premises` |
|---|---|---|---|---|---|---|---|
| `4\|a` | 25 (4/19/2) | 0 | pass (`self_sufficient true`, `evidence_bound true`) | clean (5 blocks, 4 layers) | clean | `faults: 0` | none |
| `6\|a` | 49 (8/40/1) | 0 | pass (`self_sufficient true`, `evidence_bound true`) | clean (9 blocks, 6 layers) | clean | `faults: 0` | 1 pointer (`6\|a\|a\|a` variable table noting eq. `(2)`) |
| `7\|a` | 25 (6/18/1) | 0 | pass (`self_sufficient true`, `evidence_bound true`) | clean (7 blocks, 5 layers) | clean | `faults: 0` | 1 pointer (`7\|a\|a\|a` variable table naming Table 5A col. `(5)`) |

- Term relink (`zt-digest term-link -Step relink -CitationKey babina2024entrepreneurial` under `zt_mutex.py`): `{"ok":true,"complete":true,"book":"babina2024entrepreneurial","cards_changed":7,"links_added":9,"links_repaired":0,"cards_refused":0,"rounds":1,"applied":true}`; `-PlanOnly` confirmed 0 changes (`cards_changed: 0, links_added: 0`).
- `list_broken_links` (`agent` workspace): `[]` (0 total, 0 for `babina2024entrepreneurial/`).
- Formula preservation check: all 25 unique display formulas (`4|a`: 2, `6|a`: 18, `7|a`: 5) preserved byte-for-byte (`missing: 0`).
- Tree card count: 336 -> 335 (`-1` dissolved inner chapter wrapper `6|a|a|7|a`).

## Stage 4: imports and explanations

Date: 2026-11-27, live box (`D:\zt`). Pre-Stage-4 backup: `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3`. All writes executed under `zt_mutex.py` with `-Allow` scoping; no DeepSeek calls. Order: lenders `3|a` (concept, 4 parts) and `4|a` (empirical, shared 5-source 31-state data/measurement, 25 parts), then borrowers `6|a` (establishment-panel OLS/IV, 49 own parts) and `7|a` (startup-level mechanisms, 25 own parts).

### Lenders (`3|a`, `4|a`)

- **`3|a` (concept, 4 parts):** no imports (`imports.mjs check` exit 0, `plan` `nothing to import`, 0 false shares, 0 legacy shares). Now lends `3|a|a|3` (`结论：研发催生高风险高成长且跨行业的外部 VC 初创`) as `单卡` to both `6|a` and `7|a`.
- **`4|a` (empirical, 25 parts):** no imports (`imports.mjs check` exit 0, `plan` `nothing to import`, 0 false shares, 0 legacy shares; cross-book `对照 [[guzman2020state/4|a]]` left for `zt-evidence-dialogue`). Now lends `4|a|a` (`数据：五大微观与财务数据源构建 31 州分支面板`, `整块` block of 4 cards: `4|a|a`, `4|a|a|a`, `4|a|a|b`, `4|a|a|c`) to both `6|a` and `7|a`.

### Borrowers (`6|a`, `7|a`)

1. **Hypotheses resting on `3|a`'s predictions (`单卡`):**
   - `6|a|a|1` (`假说：研发因果促创业`): replaced `参照 [[babina2024entrepreneurial/3|a]]` with `依赖 [[babina2024entrepreneurial/3|a|a|3]]` (`（经 **引入**）`) for prediction 1 (parent R&D causally raises employee entrepreneurship).
   - `7|a|a|1` (`假说：高风险多元化与契约`): replaced `参照 [[babina2024entrepreneurial/3|a]]` with `依赖 [[babina2024entrepreneurial/3|a|a|3]]` (`（经 **引入**）`) for predictions 2–3 (high-risk/VC-backed and cross-industry startups).
   - Both `6|a` and `7|a` keep `单卡 [[babina2024entrepreneurial/3|a|a|3]]` (tolerated `0 -> 2`: `3|a|a|1`, `3|a|a|2` stay in `3|a`). Because `3|a` is a concept model (`facet: 结论`), `imports.mjs plan` continues to suggest upgrading `3|a|a|3` to `整块` as advice; per the empirical-hypothesis rule (tests take the prediction, not its derivation/mechanisms), the upgrade was intentionally not applied.
2. **Shared 5-source 31-state panel and employee-entrepreneurship measure (`整块`):**
   - `6|a|a` (`数据`) and `7|a|a` (`数据`): replaced `原始设定：不依赖其他设定。` in `**起点**` with `依赖 [[babina2024entrepreneurial/4|a|a]]` and `依赖 [[babina2024entrepreneurial/4|a|a|b]]` (`（经 **引入**）`).
   - `imports.mjs apply` wrote `整块 [[babina2024entrepreneurial/4|a|a]]` (absorbing `4|a|a|b`, bringing 4 shared cards: `4|a|a`, `4|a|a|a`, `4|a|a|b`, `4|a|a|c`; 0 tolerated premises from `4|a`) into both `6|a` and `7|a`.
3. **Model Card polish & `6|a` `**结构**` repair:**
   - Refined the text after `：` on both `**引入**` lines in `6|a` and `7|a` to state specifically what each model uses.
   - Restored `6|a`'s `**结构**` bullet list (49 own parts from live `build.order`) and removed the duplicate `来源：[[babina2024entrepreneurial]] §IV.C, §V` line left over from Stage 3.
4. **Symbols (step 6):** both `6|a` and `7|a` retain their own `变量表` (`6|a|a|a`, `7|a|a|a`); `zt-check --settings` clean on all three empirical models.
5. **Explanations (step 8):** `zt-digest model-explain -Step plan -Force` returned `nothing to explain` (`parts: 0, jobs: []`) on `4|a`, `6|a`, and `7|a` (no `推导：估计式` parts).
6. **Cross-book blocks:** none written; existing cross-book `对照` links remain for `zt-evidence-dialogue`.

| Model | Import lines | Members (main/side/shared/off) | Tolerated before → after |
|---|---|---|---|
| `3\|a` | none (lender) | 4 (4/0/0/0) | 0 → 0 |
| `4\|a` | none (lender) | 25 (4/19/0/2) | 0 → 0 |
| `6\|a` | `整块 4\|a\|a` (absorbs `4\|a\|a\|b`); `单卡 3\|a\|a\|3` | 54 (8/40/5/1) | 0 → 2 (`3\|a\|a\|1`, `3\|a\|a\|2`) |
| `7\|a` | `整块 4\|a\|a` (absorbs `4\|a\|a\|b`); `单卡 3\|a\|a\|3` | 30 (6/18/5/1) | 0 → 2 (`3\|a\|a\|1`, `3\|a\|a\|2`) |

### Gate results (post term-relink)

- Term relink (`zt-digest term-link -Step relink -CitationKey babina2024entrepreneurial` under `zt_mutex.py`): `{"ok":true,"complete":true,"book":"babina2024entrepreneurial","cards_changed":2,"links_added":3,"links_repaired":0,"cards_refused":0,"rounds":1,"applied":true}`; `-PlanOnly` confirmed 0 changes (`cards_changed: 0, links_added: 0`).
- `imports.mjs check`: exit 0 on `3|a`, `4|a`, `6|a` (2 tolerated advice), `7|a` (2 tolerated advice).
- `zt-check --model '<head>' --stage final`: exit 0 on all 3 empirical models (`self_sufficient true`, `evidence_bound true`, `loose 0`, `stage(final) pass`).
- `zt-check --model '<head>' --settings`: exit 0 (`clean`) on `4|a`, `6|a`, `7|a`.
- `zt-check --model '<head>' --network`: exit 0 (`network: clean`) on `4|a` (5 blocks, 4 layers), `6|a` (11 blocks, 7 layers, both `共用` shelves), `7|a` (9 blocks, 6 layers, both `共用` shelves).
- `zt-check --model '<head>' --skip-shared`: exit 0 (`faults: 0`; advisory R20 only) on `4|a`, `6|a`, `7|a`.
- `list_broken_links` (`agent` workspace): `[]` (0 total, 0 for `babina2024entrepreneurial/`).
