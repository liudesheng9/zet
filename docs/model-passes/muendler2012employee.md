# Model passes: muendler2012employee

Owner: Xingtong Chen

Muendler, Rauch and Tocoian (2012), *Employee Spinoffs and Other Entrants: Stylized Facts from Brazil* (Scholar Archive Work 30158). Digest: `D:\zt-digest\muendler2012employee\run-report.json` (`completeness.complete: true`).

## Stage 1: fold

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

The digest left 1 Model Card. Decision by nature:

| Model Card | Model (surviving head) | Level | Reason |
|---|---|---|---|
| 7\|a | own (7\|a) | empirical | Single empirical model (64 parts): OLS comparison of initial size and 5-year exit rates across four entrant types on Brazilian RAIS 1995–2001 employer-employee microdata. |

No fold required (0 writes).

### Gate

- `model-survey.mjs muendler2012employee`: 1 Model Card (`7|a`, empirical, 64 parts).
- `zt-check --model 'muendler2012employee/7|a' --stage fold`: pass (`members 64 {main:7, off:2, side:55}`, `loose 0`, `self_sufficient true`, `evidence_bound true`).
- `list_broken_links`: verified clean (0 broken links in `muendler2012employee/`).
- Term relink: not needed (no Cards edited in Stage 1).

## Stage 2: settings

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`.

### Changes (`muendler2012employee/7|a`, empirical)

Baseline audit reported 4 `start` faults (`7|a|a|b..e` lacked `**起点**`). Added a one-line summary and `**起点**` section to each of the four data-tree Cards while keeping all formulas, numbers, facts, and existing Links intact, with no forward dependencies in data-tree order (`7|a|a` → `7|a|a|b` → `7|a|a|c` → `7|a|a|d` → `7|a|a|e`):

- `muendler2012employee/7|a|a|b` (`样本：双样本与分类规则`): added one-line summary and `**起点**` (`依赖 [[muendler2012employee/7|a|a]]`).
- `muendler2012employee/7|a|a|c` (`测量：规模、退出与明瑟工资分解`): added one-line summary and `**起点**` (`依赖 [[muendler2012employee/7|a|a]]`); display equation `$$\ln w_{it} = x_{it}\beta_t + \psi_{J(it)t} + \varepsilon_{it}$$` verified byte-for-byte unchanged.
- `muendler2012employee/7|a|a|d` (`描述：样本覆盖与就业份额（表 1–3）`): added one-line summary and `**起点**` (`依赖 [[muendler2012employee/7|a|a]]`, `依赖 [[muendler2012employee/7|a|a|b]]`).
- `muendler2012employee/7|a|a|e` (`描述：无条件均值对比（表 4）`): added one-line summary and `**起点**` (`依赖 [[muendler2012employee/7|a|a]]`, `依赖 [[muendler2012employee/7|a|a|b]]`, `依赖 [[muendler2012employee/7|a|a|c]]`).

### Gate

- `zt-digest term-link -Step relink -CitationKey muendler2012employee`: `ok: true`, `complete: true`, `cards_changed: 2`, `links_added: 2`, `links_repaired: 0`, `cards_refused: 0`; follow-up `-PlanOnly`: `cards_changed: 0`, `links_added: 0` (rc=0).
- `zt-check --model 'muendler2012employee/7|a' --settings`: pass (`settings: data tree and specification ladder are clean`, rc=0).
- `zt-check --model 'muendler2012employee/7|a' --stage settings`: pass (`members 64 {main:7, off:2, side:55}`, `relations {contrasts:3, refers:116, rests-on:29}`, `loose 0`, `self_sufficient true`, `evidence_bound true`, rc=0).
- `list_broken_links`: `broken: []` (0 broken links).

## Stage 3: restructure

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`.

### Changes (`muendler2012employee/7|a`, empirical)

Baseline audit reported 2 `lead` faults (`7|a|a|1` and `7|a|a|2` lacked `**直觉**` leads); `--stage restructure`, `--settings`, and `--skip-shared` already passed, and no moves were required. Added crisp `**直觉**` leads (`>= 20` chars) to both cards while keeping all display formulas, equation numbers, and estimates byte-for-byte unchanged:

- `muendler2012employee/7|a|a|1` (`设计：同四位行业与同进入队列内的横截面 OLS 组间比较`): added `**直觉**` lead (44 chars) stating how within-industry, within-cohort cross-sectional OLS strips out industry and macro shocks to establish the baseline for isolating spinoff survival advantages.
- `muendler2012employee/7|a|a|2` (`规格：五人及以上员工样本五年退出基准 OLS 回归（表 5 第 4 列 / 表 7 第 1 列）`): added `**直觉**` lead (51 chars) stating how the baseline OLS linear probability model compares 5-year exit across the four entrant types net of industry and cohort fixed effects as the starting point for the control ladder; display formula `$$ Exit_{i,t+5} = \beta_1 D_i^{spin} + \beta_2 D_i^{div} + \beta_3 D_i^{dv} + \delta_{c(i)} + \gamma_t + u_i $$` verified byte-for-byte unchanged.

Verified the 2 `--premises` hits (`H|a|1|c` and `H|a|2|c|a|b`) against `H|a|2|c|a|b|a` (`规格：表 6 第(8)列加入母公司与明瑟工资成分交互项`): both are pure table-column `(8)` pointers (`表 6 第 (3)–(4)、(7)–(8) 列` and `表 6 第(8)列`), not hidden forward equation uses; both stay as written.

### Gate

| Check | Result | Exit |
|---|---|---|
| `zt-digest term-link -Step relink -CitationKey muendler2012employee` | `ok: true`, `complete: true`, `cards_changed: 0`, `links_added: 0`; `-PlanOnly` = 0 changes | 0 |
| `zt-check --model 'muendler2012employee/7|a' --stage restructure` | `members 64 {main:7, off:2, side:55}`, `relations {contrasts:3, refers:116, rests-on:29}`, `loose 0`, `self_sufficient true`, `evidence_bound true`, `stage(restructure) pass` | 0 |
| `zt-check --model 'muendler2012employee/7|a' --network` | `network: clean` (8 blocks, 5 layers, 414px wide, 15 paths) | 0 |
| `zt-check --model 'muendler2012employee/7|a' --settings` | `settings: data tree and specification ladder are clean` | 0 |
| `zt-check --model 'muendler2012employee/7|a' --skip-shared` | `faults: 0` (4 R20 advisory notices on `**本章估计**`) | 0 |
| `zt-check --model 'muendler2012employee/7|a' --premises` | 2 hits verified as pure table-column `(8)` pointers (`H|a|1|c`, `H|a|2|c|a|b`), 0 hidden equation uses | 1 (advisory pointer hits) |
| `list_broken_links` (`muendler2012employee`) | `broken: []` (0 broken links) | 0 |
