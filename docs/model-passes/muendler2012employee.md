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
