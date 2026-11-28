# shaw2019productivity: post-digest model passes

Owner: Xingtong Chen

Shaw, Kathryn, and Anders Sørensen (2019), *The Productivity Advantage of Serial Entrepreneurs*, *ILR Review* 72(5), 1225–1246. Post-digest sequence (`zt-post-digest`) on the live box `D:\zt`.

Pre-Stage-1 backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3`.

## Stage 1: fold

Date: 2026-11-27 (UTC 2026-11-28), live box `D:\zt`.

Survey before: 6 Model Cards (all `empirical`), 6 candidate groups (`G1: 3|a`, `G2: 4|a`, `G3: 5|a`, `G4: 5|b`, `G5: 6|a`, `G6: 6|b`); no `rests-on` Link or legacy share joined any two (`read` pairs: `3|a -> 4|a: kin 1`, `5|a -> 4|a: kin 1`, `6|a -> 4|a: kin 1`).

### Decision table

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| `shaw2019productivity/3\|a` | §3 | Serial entrepreneurship sales & TFP panel regression (eqs 1 & 3) | `shaw2019productivity/5\|a` | empirical; §3 empirical framework setting up the 2001–2013 semi-annual panel, 6-year truncation right-censoring stats, hypotheses H1–H4, identification assumptions, and estimating equations (1)–(3) estimated by `5\|a` and `6\|a`, not an independent empirical model (folds into `5\|a`, which carries the main Table 2 & firm-order estimates and cross-book link to `levine2017smart/6\|a`) |
| `shaw2019productivity/4\|a` | §4 (Table 1, App. Table A.1) | Danish 5-registry panel construction, sample screening, Table 1 descriptives & LPM of serial selection | `shaw2019productivity/4\|a` | empirical; standalone shared measurement/descriptive model building the 5-registry Danish administrative panel consumed by both `5\|a` (semi-annual) and `5\|b` (annual, to be imported via `**引入**` in Stage 4), with its own distinct estimands (24-yr vs 13-yr serial prevalence, Table 1 group means, and LPM predicting `Serial` status) |
| `shaw2019productivity/5\|a` | §5.2–§5.3 (Fig. 1, Table 2, firm-order) | Serial entrepreneurship sales & TFP panel regression (eqs 1 & 3) | `shaw2019productivity/5\|a` | empirical; main estimation head for the semi-annual $\log S_{ijt}$ / TFP panel ($N = 757{,}143$, 128,537 firms, first 6 years): Fig. 1 sales distribution, Table 2 OLS(1)–(3) (+98% sales, +49% TFP) & FE(4)–(5) within-firm learning, and firm-order regression (Firm 1 vs Firm 2) addressing reverse causality |
| `shaw2019productivity/5\|b` | §5.4 (Table 3, App. Table A.12) | Serial entrepreneurship employment panel regression (eq 2) | `shaw2019productivity/5\|b` | empirical; distinct estimand ($\log L_{ijt}$, total FTE employment and within-firm hiring expansion — $\log L$ shifts from RHS control in eq 1 to LHS outcome in eq 2) on a different panel frequency and observation set (annual General Enterprise Statistics panel, $N = 416{,}690$ firm-year obs vs semi-annual $N = 757{,}143$), plus Ch 0 employer background stats |
| `shaw2019productivity/6\|a` | §6–§6.2 (Tables 4–6) | Serial entrepreneurship sales & TFP panel regression (eqs 1 & 3) | `shaw2019productivity/5\|a` | empirical; heterogeneity (Table 4 gender, Table 5 sequential vs portfolio; eq 3 is eq 1 stratified by subgroup $k$ on the same $N = 757{,}143$ semi-annual panel) and LLC/SP mechanism proxy check (Table 6) of `5\|a` (per `docs/empirical-models-proposal.md` §9.1, heterogeneity and mechanisms belong to the same empirical model) |
| `shaw2019productivity/6\|b` | §6 (Table 3, endnote 18) | Serial entrepreneurship employment panel regression (eq 2) | `shaw2019productivity/5\|b` | empirical; exact duplicate split of `5\|b` (same Table 3, eq 2, $N = 416{,}690$ annual obs, OLS 1–3 & FE 4–5) emitted because Table 3 floated into §6 in the source layout; folds into `5\|b` (Stage 2/3 will cleanly deduplicate overlapping parts) |

The book holds **3 empirical models**: `shaw2019productivity/4|a` (shared 5-registry data construction, Table 1 descriptives & LPM of serial selection), `shaw2019productivity/5|a` (semi-annual sales & TFP panel regression, heterogeneity & LLC mechanism), and `shaw2019productivity/5|b` (annual employment panel regression).

### Fold actions

Three folds executed under `zt_mutex.py` (no duplicate parts deleted in Stage 1; no body text rewritten beyond stripping `**结构**` and masking/restoring model-part Links for retype):

1. **`shaw2019productivity/3|a` → `shaw2019productivity/5|a`** (moved to `shaw2019productivity/5|a|a|7`):
   - Stripped `**结构**` (no `**引入**` section, 0 Idea-note Side successors).
   - Parked `3|a|a` and `3|a|b` on `5|a` (`5|a|c`, `5|a|d`); masked 4 model-part Links in `3|a`; retyped `3|a` (`facet: "第 4 章：实证框架（式 1–3）与右删失设定"`, `emphasis: ""`, `level: ""`); restored all 4 Links.
   - Moved `3|a` alone after `5|a|a|6` → `5|a|a|7` and restored its 22-card subtree under `5|a|a|7|a` and `5|a|a|7|b` in one atomic `tidy` batch.
2. **`shaw2019productivity/6|b` → `shaw2019productivity/5|b`** (moved to `shaw2019productivity/5|b|a|5`):
   - Stripped `**结构**` (no `**引入**` section, 0 Idea-note Side successors).
   - Parked `6|b|a` and `6|b|b` on `5|b` (`5|b|c`, `5|b|d`); masked 6 model-part Links in `6|b`; retyped `6|b` (`facet: "第 2 章：就业面板回归全表（表 3）"`, `emphasis: ""`, `level: ""`); restored all 6 Links.
   - Moved `6|b` alone after `5|b|a|4` → `5|b|a|5` and restored its 19-card subtree under `5|b|a|5|a` and `5|b|a|5|b` in one atomic `tidy` batch.
3. **`shaw2019productivity/6|a` → `shaw2019productivity/5|a`** (moved to `shaw2019productivity/5|a|a|8`):
   - Stripped `**结构**` (no `**引入**` section, 0 Idea-note Side successors).
   - Parked `6|a|a` and `6|a|b` on `5|a` (`5|a|c`, `5|a|d`); masked 10 model-part Links in `6|a`; retyped `6|a` (`facet: "第 5 章：性别、创业方式异质性与公司制（表 4–6）"`, `emphasis: ""`, `level: ""`); restored all 10 Links.
   - Moved `6|a` alone after `5|a|a|7` → `5|a|a|8` and restored its 47-card subtree under `5|a|a|8|a` and `5|a|a|8|b` in one atomic `tidy` batch.

### Surviving heads

| Head | Level | Parts | Main line |
|---|---|---|---|
| `shaw2019productivity/4\|a` | `empirical` | 23 (`main: 4, side: 18, off: 1`) | `H\|a` 数据 → `H\|a\|1` 设计 → `H\|a\|2` 规格：分组均值 → `H\|a\|3` 第 1 章：样本构成与表 1 描述性均值 |
| `shaw2019productivity/5\|a` | `empirical` | 105 (`main: 9, side: 95, off: 1`) | `H\|a` 数据 → `H\|a\|1` 设计 → `H\|a\|2` 规格：基准 → `H\|a\|3` 第 0 章：销售额分布的描述性特征 → `H\|a\|4` 第 1 章：销售额与全要素生产率优势 → `H\|a\|5` 第 2 章：企业内学习曲线与质量筛选偏误 → `H\|a\|6` 第 3 章：分企业回归检验反向因果 → `H\|a\|7` 第 4 章：实证框架（式 1–3）与右删失设定 → `H\|a\|8` 第 5 章：性别、创业方式异质性与公司制（表 4–6） |
| `shaw2019productivity/5\|b` | `empirical` | 41 (`main: 6, side: 34, off: 1`) | `H\|a` 数据 → `H\|a\|1` 设计 → `H\|a\|2` 规格：基准 → `H\|a\|3` 第 0 章：雇主型企业背景统计 → `H\|a\|4` 第 1 章：就业起步差异与观望扩张动态 → `H\|a\|5` 第 2 章：就业面板回归全表（表 3） |

### Gate

- `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs shaw2019productivity`: 3 Model Cards (`4|a`, `5|a`, `5|b`), 3 candidate groups (`G1: 4|a`, `G2: 5|a`, `G3: 5|b`), matching the decided surviving heads.
- `zt-check --model 'shaw2019productivity/4|a' --stage fold`: exit 0 (`members 23`, `loose 1`, `self_sufficient true`, `evidence_bound true`, `stage(fold) pass`).
- `zt-check --model 'shaw2019productivity/5|a' --stage fold`: exit 0 (`members 105`, `loose 2`, `stage(fold) pass`; only expected deferred notices: Stage 2 `spec_unbound` ×2 on `5|a|a|7|a|6` and `5|a|a|8|a|2`, Stage 3 `unanchored` ×2 on `5|a|a|7` and `5|a|a|8`).
- `zt-check --model 'shaw2019productivity/5|b' --stage fold`: exit 0 (`members 41`, `loose 2`, `stage(fold) pass`; only expected deferred notices: Stage 2 `spec_unbound` ×1 on `5|b|a|5|a|2`, Stage 3 `unanchored` ×1 on `5|b|a|5`).
- `list_broken_links` (`prefix: shaw2019productivity/`): 0 broken links.
- Tree card count: 315 Cards (unchanged).
- `zt-digest term-link -Step relink -CitationKey shaw2019productivity` (under `zt_mutex.py`) and `-PlanOnly`: `ok: true`, `complete: true`, `cards_changed: 0`, `links_added: 0`, `links_repaired: 0`, `cards_refused: 0`.

### Next stages

All three surviving heads (`shaw2019productivity/4|a`, `shaw2019productivity/5|a`, `shaw2019productivity/5|b`) are `empirical` models and proceed to Stage 2 (`zt-model-settings`, including consolidating the folded data/spec/variable cards in `5|a` and deduplicating the folded Table 3 parts in `5|b`), Stage 3 (`zt-model-restructure`), Stage 4 (`zt-model-import`, where `5|a` and `5|b` import `4|a`'s shared 5-registry administrative data/measurement blocks), and Stage 6 (`zt-evidence-dialogue`), followed by Stage 7 (`zt-literature-organize`). Stage 5 (`zt-model-illustrate`) does not apply (no formal models).


## Stage 2: settings

Date: 2026-11-27 (UTC 2026-11-28), live box `D:\zt`.

Pre-Stage-2 backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`.

All writes serialized via `zt_mutex.py`; every display formula and equation number (`\tag{1}`, `\tag{2}`, `\tag{3}`, and all Table 2/3/4/5/6 specs) verified byte-for-byte before writing.

### `shaw2019productivity/4|a` (empirical, 23 → 23 parts)

- Fixed 2 `start` faults on `4|a|a|b` (`测量：活跃初创与连续创业者`) and `4|a|a|c` (`样本：全样本与主回归样本`): restructured each with a one-line summary, `**起点**` (`- 依赖 [[shaw2019productivity/4|a|a]]：…`), and `**内容**`, dropping the redundant leading `见 [[shaw2019productivity/4|a|a]]。`. No cards moved.

### `shaw2019productivity/5|a` (empirical, 105 → 95 parts)

- Dissolved the folded setting blocks of `5|a|a|7` (former `3|a`, §3 framework) and `5|a|a|8` (former `6|a`, §6 Tables 4–6) in one atomic `apply_batch` (`tidy`, 57 cards moved, 10 redundant duplicates deleted `into` surviving targets, `links_that_will_break: []`):
  - **Deleted `into` surviving counterparts (10 cards):** `7|a` and `8|a` (`数据`) into `5|a|a`; `7|a|a` and `8|a|a` (`变量表`) into `5|a|a|a`; `7|a|5` and `8|a|1` (`设计`) into `5|a|a|1`; `7|a|5|a` (`假设 1`) into `5|a|a|1|a`; `7|a|5|b` and `8|a|1|b` (`假设 2`) into `5|a|a|1|b`; `7|a|5|c` (`假设 3`) into `5|a|a|1|c`.
  - **Moved into unified data tree & design:** `7|a|b` → `5|a|a|b` (`测量：创业决策代理`) and `8|a|b` → `5|a|a|c` (`测量：顺序型与组合型占比`), both given one-line summary, `**起点**`, and `**内容**`; distinct identifying assumptions `7|a|5|d` → `5|a|a|1|d` (`假设 4`), `8|a|1|a` → `5|a|a|1|e` (renumbered `假设 5`), and `8|a|1|c` → `5|a|a|1|f` (renumbered `假设 6`).
  - **Moved onto unified specification ladder:** `7|a|6` (+ 4 Side variants including eqs (1)–(3)) → `5|a|a|2|b` (facet `规格：式 (1) 完整框架`, preserving `\tag{1}`–`\tag{3}`); `8|a|2` (+ 17 Side variants for Tables 4–6) → `5|a|a|2|c` (facet `规格：男性基准 OLS(1)`). Both converted to variants of `5|a|a|2` with `依赖 [[shaw2019productivity/5|a|a|2]]` and `**相对基准的改动**`.
  - **Promoted folded chapter content ahead of `局限`:** `7|a|1–4` + `7|a|7` → `5|a|a|7|a` chain ahead of `5|a|a|7|b`; `8|a|3–5` → `5|a|a|8|a` chain ahead of `5|a|a|8|b`.
- Merged subsample sizes and 6-year right-censoring notes into `5|a|a`, unified design/threat/assumption list (Assumptions 1–6) into `5|a|a|1`, enriched `5|a|a|1|a–c`, and added the 7 new symbol rows ($\alpha, \Sigma, \Phi, \zeta_{ijt}, Share_{rc}, Count_{rc}, Total_c$) to `5|a|a|a` (`变量表`, 24 rows, 0 `uncovered`/`unused`).
- Rebuilt `**结构**` on `shaw2019productivity/5|a` (95 parts).

### `shaw2019productivity/5|b` (empirical, 41 → 31 parts)

- Dissolved the exact-duplicate Table 3 setting block inside folded chapter `5|b|a|5` (former `6|b`) in one atomic `apply_batch` (`tidy`, 52 link macros rewritten, `links_that_will_break: []`):
  - Moved `5|b|a|5|a|3` (+ 7 Side estimate/table/test children) → `5|b|a|5|a` ahead of `5|b|a|5|b` (`局限`).
  - Deleted 10 exact-duplicate cards leaf-first `into` their surviving counterparts: `5|b|a|5|a|2*` (5 specs, all display formulas verified identical) into `5|b|a|2*`; `5|b|a|5|a|1|a–b` (`假设 1–2`) into `5|b|a|1|a–b`; `5|b|a|5|a|1` (`设计`) into `5|b|a|1`; `5|b|a|5|a|a` (`变量表`) into `5|b|a|a`; `5|b|a|5|a` (`数据`) into `5|b|a`.
- Enriched `5|b|a`, `5|b|a|1`, and `5|b|a|1|b` with minor unique details (1990–2000 exclusion, conditional-correlation threat, endnote 18 citation). Duplicate estimate-chapter consolidation (`5|b|a|5` vs `5|b|a|4`) deferred to Stage 3.
- Rebuilt `**结构**` on `shaw2019productivity/5|b` (31 parts).

### Gate

- Term relink (`zt-digest term-link -Step relink -CitationKey shaw2019productivity` under `zt_mutex.py`): `{"ok":true,"complete":true,"book":"shaw2019productivity","cards_changed":3,"links_added":10,"links_repaired":0,"cards_refused":0,"rounds":1,"applied":true}`; `-PlanOnly` confirmed `0` pending changes.
- `zt-check --model 'shaw2019productivity/4|a' --settings`: exit 0 (`settings: data tree and specification ladder are clean`).
- `zt-check --model 'shaw2019productivity/4|a' --stage settings`: exit 0 (`members 23 {main:4, off:1, side:18}`, `relations {refers:39, rests-on:8}`, `loose 1`, `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`).
- `zt-check --model 'shaw2019productivity/5|a' --settings`: exit 0 (`settings: data tree and specification ladder are clean`).
- `zt-check --model 'shaw2019productivity/5|a' --stage settings`: exit 0 (`members 95 {main:9, off:1, side:85}`, `relations {contrasts:4, refers:177, rests-on:31}`, `loose 2`, `evidence_bound true`, `stage(settings) pass`; only expected Stage 3 deferred `unanchored` on `5|a|a|7` and `5|a|a|8`).
- `zt-check --model 'shaw2019productivity/5|b' --settings`: exit 0 (`settings: data tree and specification ladder are clean`).
- `zt-check --model 'shaw2019productivity/5|b' --stage settings`: exit 0 (`members 31 {main:6, off:1, side:24}`, `relations {refers:66, rests-on:10}`, `loose 2`, `evidence_bound true`, `stage(settings) pass`; only expected Stage 3 deferred `unanchored` on `5|b|a|5`).
- `list_broken_links` (`prefix: shaw2019productivity/`): `{"broken":[]}` (0 broken links).
- Tree card count: 315 → 295 Cards (−20 redundant folded duplicate cards deleted `into` surviving targets).


## Stage 3: restructure

Date: 2026-11-27 (UTC 2026-11-28), live box `D:\zt`.

Pre-Stage-3 backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`.

All writes serialized via `zt_mutex.py`; every unique display formula `$$…$$`, equation number (`\tag{1}`, `\tag{2}`, `\tag{3}`), and empirical estimate number preserved byte-for-byte.

### `shaw2019productivity/4|a` (empirical, 23 → 23 parts, 0 moves)

- Added crisp `**直觉**` leads (≥20 chars) to main-line cards `4|a|a` (`数据`), `4|a|a|1` (`设计`), and `4|a|a|2` (`规格：分组均值`).
- Fixed 3 `R22` bare-LaTeX faults on `4|a|a|2|a` (`$Serial_{ij}$`, `$X_{ij}$`, `$Z_{ij}$`).
- Anchored off-spine limitation card `4|a|b` with plain pointer `见设计 [[shaw2019productivity/4|a|a|1]]。` (`loose` 1 → 0).

### `shaw2019productivity/5|a` (empirical, 95 → 93 parts)

- Dissolved the two folded model-card wrappers (`5|a|a|7`, former `3|a`, and `5|a|a|8`, former `6|a`) in one atomic `apply_batch` (`tidy`, 88 cards moved, 2 wrapper cards deleted `into` their promoted chapters, `links_that_will_break: []`):
  - **Placed §3 hypotheses in the evidence spine's `假说` slot:** folded `H2`–`H4` (`7|a|1..3`) as Side chain under `H1` (`7|a`) and moved `7|a` onto the main line right after `数据` → `5|a|a|1` (`假说：开业优势`, with `H2`–`H4` at `5|a|a|1|a..a|2`).
  - **Promoted 4 inner chapters directly onto the main line:** `7|a|4` → `5|a|a|8` (`第 4 章：样本存续期与右删失测算`, carrying `7|b` limitation as Side `8|c`), `8|a` → `5|a|a|9` (`第 5 章：按性别分样本的销售额与生产率`, Table 4), `8|a|1` → `5|a|a|10` (`第 6 章：顺序型与组合型对比`, Table 5), `8|a|2` → `5|a|a|11` (`第 7 章：有限责任公司与机制检验`, Table 6, carrying `8|b` limitation as Side `11|e`).
  - **Deleted redundant wrappers `into` promoted chapters:** `5|a|a|7` (which duplicated `\tag{1}`–`\tag{3}` already carried on `5|a|a|3|b`, `3|b|c`, `3|b|d` and triggered `R13`) `into` `第 4 章`; `5|a|a|8` `into` `第 5 章`.
- Added `**直觉**` leads (≥20 chars) to `5|a|a` (`数据`), `5|a|a|1` (`假说`), `5|a|a|2` (`设计`), and `5|a|a|3` (`规格：基准`); updated facets, titles, and `**承上启下**` on `第 4–7 章`.
- Cleared all 13 blocking `--skip-shared` faults (`R2`/`R3` on `5|a|a` and `5|a|a|c`, `R22` bare LaTeX on `5|a|a|3|b|b`, `3|b|c`, `3|c|b|a`, and `R13` via dissolving `5|a|a|7`).
- Anchored all limitation cards (`5|a|a|8|c` → `第 4 章`, `5|a|a|11|e` → `第 7 章`, `5|a|b` → `设计 5|a|a|2`; `loose` 2 → 0).
- Rebuilt `**结构**` on `shaw2019productivity/5|a` (93 parts) and added `**分部**` (3 parts covering 8 chapters: `第一部分 设定与假说` from `5|a|a`, `第二部分 基准回归与学习曲线` from `5|a|a|4`, `第三部分 样本删失、异质性与机制` from `5|a|a|8`).

### `shaw2019productivity/5|b` (empirical, 31 → 22 parts)

- Consolidated duplicate Table 3 chapter `5|b|a|5` (former `6|b`) into `5|b|a|4` (`第 1 章：就业起步差异与观望扩张动态`):
  - **Preserved all unique numbers before deduplication:** merged the 5 extra personal-background control rows (`Married`, `Education`, `Male`, `Prior business experience`, `Age`) from `5|b|a|5|a|a` into `5|b|a|4|a` (`表 3`), enriched `5|b|a|4|f` with `FE(5)` novice/quadratic coefficients (`0.184`, `-0.018`, `-0.029`), and preserved the distinct `OLS(2)` estimate card `5|b|a|5|a|c` (`Serial = -0.185`) as Side branch `5|b|a|4|g` (added to `5|b|a|4`'s `**本章估计**`).
  - **Atomic `tidy` batch:** moved `5|b|a|5|a|c` → `5|b|a|4|g` and deleted 9 exact-duplicate cards leaf-first `into` their surviving twins (`5|a|e|a` → `4|e|a`, `5|a|f` → `4|f`, `5|a|e` → `4|e`, `5|a|d` → `4|c`, `5|a|b` → `4|b`, `5|a|a` → `4|a`, `5|a` → `4`, `5|b` → `5|b|b`, `5` → `4`).
- Added `**直觉**` leads (≥20 chars) to `5|b|a` (`数据`), `5|b|a|1` (`设计`), and `5|b|a|2` (`规格：基准`); cleared all 7 blocking `--skip-shared` faults (`R3` semicolons on `5|b|a` and `5|b|a|1`, `R22` bare LaTeX on `5|b|a|2|a`, `2|a|a`, `2|a|b`, `2|a|b|a`).
- Anchored `5|b|b` (`见设计 [[shaw2019productivity/5|b|a|1]]。`; `loose` 2 → 0) and rebuilt `**结构**` on `shaw2019productivity/5|b` (22 parts).

### Gate

Term relink (`zt-digest term-link -Step relink -CitationKey shaw2019productivity` under `zt_mutex.py`): `{"ok":true,"complete":true,"book":"shaw2019productivity","cards_changed":4,"links_added":11,"links_repaired":0,"cards_refused":0,"rounds":1,"applied":true}`; `-PlanOnly` confirmed `0` pending changes. All gates below were run **after** term-relink:

| Model | parts (main/side/off) | loose | `--stage restructure` | `--network` | `--settings` | `--skip-shared` | `--premises` |
|---|---|---|---|---|---|---|---|
| `4\|a` | 23 (4/18/1) | 0 | pass (rc=0, `self_sufficient true`) | `network: clean` (5 blocks, 4 layers) | clean (rc=0) | `faults: 0` (rc=0) | none (rc=0) |
| `5\|a` | 93 (12/80/1) | 0 | pass (rc=0, `self_sufficient true`) | `network: clean` (13 blocks, 4 layers, 3 parts) | clean (rc=0) | `faults: 0` (rc=0) | 9 pointers (0 hidden uses) |
| `5\|b` | 22 (5/16/1) | 0 | pass (rc=0, `self_sufficient true`) | `network: clean` (6 blocks, 4 layers) | clean (rc=0) | `faults: 0` (rc=0) | 2 pointers (0 hidden uses) |

- `--premises` audit (0 hidden forward uses):
  - `5|a` (9 pointers): 3 in `变量表` (`5|a|a|a`) naming eqs `(1)`–`(3)`; 4 in hypotheses `H1`–`H4` (`5|a|a|1`, `1|a`, `1|a|1`, `1|a|2`) pointing forward to the estimating equations `(1)`–`(3)` that operationalize them; 2 in `设计` (`5|a|a|2`) and `假设 1` (`5|a|a|2|a`) pointing to eq `(3)` and `(1)`.
  - `5|b` (2 pointers): `假设 1` (`5|b|a|1|a`) pointing forward to Table 3 columns `FE(4)` and `FE(5)` (`- 怎样进入模型：规格 FE(4) 与 FE(5) 纳入 \lambda_j…`).
- `list_broken_links` (`prefix: shaw2019productivity/`): `{"broken":[]}` (0 broken links).
- Tree card count: 295 → 284 Cards (−11: −2 folded model-card wrappers in `5|a`, −9 exact-duplicate Table 3 cards in `5|b`, all deleted `into` surviving targets).
