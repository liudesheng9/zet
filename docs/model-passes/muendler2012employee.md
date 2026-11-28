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

## Stage 4: imports and explanations

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

- Imports (`muendler2012employee/7|a`): single self-contained empirical model in the book; `imports.mjs check` and `imports.mjs plan` report `nothing to import` (`0` tolerated premises, `0` false shares, `0` legacy shares).
- Explanation refresh (Step 8): `zt-digest model-explain -Step plan -Model 'muendler2012employee/7|a'` returned `nothing to explain` (`parts: 0`, `jobs: []`).

### Gate

- `imports.mjs check 'muendler2012employee/7|a'`: exit 0 (`takes what it uses from other models by import lines`).
- `zt-check --model 'muendler2012employee/7|a' --stage final`: pass (`self_sufficient true`, `evidence_bound true`, `loose 0`).
- `zt-check --model 'muendler2012employee/7|a' --skip-shared`: `faults: 0`.
- `list_broken_links`: 0 broken.
- Term relink: not needed (0 Cards edited).

## Stage 5: evidence dialogue

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage5-dialogue-2026-11-27.sqlite3`.

- `dialogue.mjs survey 'muendler2012employee/7|a'`: `ours: 17`, `theirs: 2693`, `pairs: 0` (0 candidate pairs; dialogue complete with no edits needed).
- Gate: `dialogue.mjs check` not needed / clean (`0` pairs proposed).

## Stage 6: literature organize

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage6-organize-2026-11-27.sqlite3`.

### Source divisions

From `muendler2012employee.source_headings.txt`: Front matter (`论文信息`, `摘要`); `1. 引言`; `2. 相关文献`; `3. 数据`; `4. 新进入者类型的界定` (`4.1`–`4.4`); `5. 员工衍生企业、其他新企业与多元化新设业务的统计`; `6. 员工衍生企业的绩效表现`; `7. 结论`; `附录 A. 雇主—雇员数据` (`员工数据筛选`, `收入`, `职业分类`); `附录 B. 企业识别码`; `附录 C. 法律形式（Natureza Juridica）`; `附录 D. 衍生企业与剥离分拆企业界定的实证实施` (preamble + 8 subsections); followed by `术语表` and `参考文献` at the tail.

### Final tree (top line, 14 Cards, 235 total)

```
1  摘要与论文信息 (4; 1|a 论文信息 · 1|b 摘要 · 1|c 结构卡)
2  1. 引言 (5)
3  2. 相关文献 (4)
4  3. 数据 (9; events 4|a|2|a|a, 4|a|2|a|b)
5  4. 新进入者类型的界定 (13; 5|a 4.1 + finding · 5|e 4.2 · 5|f 4.3 · 5|g 4.4 + 2 findings)
6  5. 员工衍生企业、其他新企业与多元化新设业务的统计 (17; 6|a chain + 9 findings)
7  6. 员工衍生企业的绩效表现 (92; empirical Model Card 7|a [64 parts] · 7|b chain · 12 findings)
8  7. 结论 (2; old 8|a promoted to top card + 8|a [old 8|a|1])
9  附录 A. 雇主—雇员数据 (6; old 8; 9|b 员工数据筛选 · 9|c 收入 · 9|d 职业分类)
10 附录 B. 企业识别码 (4; old 9; 10|a..10|a|2)
11 附录 C. 法律形式（Natureza Juridica） (5; old 10)
12 附录 D. 衍生企业与剥离分拆企业界定的实证实施 (13; old 11; 12|a 追溯框架 · 12|e..12|k 7 subdivision branches with findings)
13 术语表 (30; from 1|b)
14 参考文献 (30; from 1|a)
```

### Moves, crossers, and leads

- 4 atomic batches (`batch.mjs --commit`, tail-to-head, all previews `complete true, refusals [], breaks 0`):
  1. `01_appD.ops` (10 ops, 11 renamed, 67 rewrites): nested Appendix D subdivisions and findings under `11`.
  2. `02_sec7_appAB.ops` (5 ops, 30 renamed, 117 rewrites): settled crossers `8` (`结论与附录 A`) and `9` (`附录 A 与附录 B`) — promoted `8|a` + `8|a|1` into `8` (`7. 结论`), moved `8|a|2..8|a|4` and `9|a`, `9|a|1` into old `8` → `9` (`附录 A. 雇主—雇员数据`, preserving inbound Appendix A links), and left `9|a|2..9|a|4` under old `9` → `10` (`附录 B. 企业识别码`, preserving inbound Appendix B links).
  3. `03_sec4.ops` (6 ops, 10 renamed, 27 rewrites): nested `§4.1`–`§4.4` and findings under `5`.
  4. `04_front_and_lists.ops` (4 ops, 63 renamed, 712 rewrites): moved Term list (`1|b`) then Cited list (`1|a`) to tail (`13`, `14`), split `论文信息` (`1|a`) and `摘要` (`1|b`) before structure Card `1|c`.
- Whole Model Card `muendler2012employee/7|a` (empirical, 64 parts) stayed intact at `7|a`.
- Summary leads (`leads.mjs --commit`): rewrote `1`, `5`, `9`, `10`, `12` with `小节索引：` (where subdivided), `在全书中的位置：`, and `来源：`. Updated root index (`muendler2012employee`) and structure Card (`1|c`), added section-number title prefixes on `4`, `6`, `7`, `8`, fixed stale conclusion link in `7` (`9` → `8`) and Appendix A income links in Cited Card `14|o` (`10` → `9|c`).

### Gate (post term-relink)

| Check | Result | Exit |
|---|---|---|
| `zt-digest term-link -Step relink` & `-PlanOnly` | `ok: true`, `cards_changed: 0`, `links_added: 0` | 0 |
| `zt-check --tree muendler2012employee --snapshot` | `no fault` (14 top Cards, 235 Cards unchanged) | 0 |
| `zt-check --cards` (root, `1`, `1\|c`, `4`, `5`, `6`, `7`, `8`, `9`, `10`, `12`) | `faults: 0` (advisory L1/L2 only) | 0 |
| `model-survey.mjs muendler2012employee` | 1 empirical Model Card: `7\|a` (64 parts) | 0 |
| `zt-check --model 'muendler2012employee/7|a' --stage final` | `stage(final) pass` (`members 64`, `loose 0`, `self_sufficient true`, `evidence_bound true`) | 0 |
| `zt-check --model 'muendler2012employee/7|a' --settings` | `settings: data tree and specification ladder are clean` | 0 |
| `zt-check --model 'muendler2012employee/7|a' --network` | `network: clean` (8 blocks, 5 layers, 414px wide, 15 paths) | 0 |
| `list_broken_links` (`muendler2012employee/`) | `{"broken":[]}` | 0 |
