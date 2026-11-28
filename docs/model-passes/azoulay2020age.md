# azoulay2020age: post-digest model passes

Owner: Xingtong Chen

Azoulay, Jones, Kim, and Miranda (2020), *Age and High-Growth Entrepreneurship*, *American Economic Review: Insights* 2(1), 65–82. Post-digest sequence (`zt-post-digest`) on the live box `D:\zt`.

Pre-Stage-1 backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3`.

## Stage 1: fold

Date: 2026-11-27 (UTC 2026-11-28), live box `D:\zt`.

Survey before: 3 Model Cards (all `empirical`), 3 candidate groups (`G1: 2|a`, `G2: 3|a`, `G3: 5|a`); no `rests-on` Link or legacy share joined any two (`read` pair: `2|a -> 5|a: kin 1`).

### Decision table

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| `azoulay2020age/2\|a` | §1 | Administrative data construction and cross-legal-form founder-identification validation | `azoulay2020age/2\|a` | empirical; standalone measurement/validation model with its own estimands ($p_{\text{no-worker}} \approx 20\%$ S-corps with no owner-worker; $p_{\text{top3}} = 90\%$ owner-worker top-3 W-2 coverage) and shared de novo firm, founder, ex-ante tech, and ex-post high-growth/exit definitions consumed by both `3\|a` and `5\|a` (to be imported via `**引入**` in Stage 4) |
| `azoulay2020age/3\|a` | §2, §2.1–§2.2 (Tables 1–2) | Stratified mean founder age across geography, ex-ante tech orientation, and ex-post upper-tail success | `azoulay2020age/3\|a` | empirical; distinct descriptive estimand $E[\text{Age} \mid \text{region}, \text{tech}, \text{success}]$ (unconditional/stratified subsample means and SDs in Tables 1–2 plus sole-proprietor, 3/7-year window, industry, year, legal-form, and team-age robustness) |
| `azoulay2020age/5\|a` | §2.3–§2.4 (Figures 1–2) | Conditional LPM of extreme startup success on single-year founder age fixed effects | `azoulay2020age/5\|a` | empirical; distinct conditional estimand $\Pr(\text{Success}_i = 1 \mid \text{Age}_i = a) - \Pr(\text{Success}_i = 1 \mid \text{Age}_i \le 20)$ ("batting average" via OLS LPM on single-year age FEs with robust SEs for M&A/IPO exit and top-0.1% 5-year employment growth, bridged from Figure 1's age distribution and checked with industry FE, sales growth, and prior industry experience/wage mechanisms) |

The book holds **3 empirical models**: `azoulay2020age/2|a`, `azoulay2020age/3|a`, and `azoulay2020age/5|a`. Under `zt-model-fold` (empirical rule, `docs/empirical-models-proposal.md` §9.1), `2|a` is a shared measurement/validation model with its own S-corp validation estimands rather than measurement private to `3|a`, while `3|a` ($E[\text{Age} \mid S]$) and `5|a` ($E[\text{Success} \mid \text{Age}]$) estimate fundamentally different estimands on the same linked administrative data.

### Fold actions

None required (0 folds executed; no Cards edited, retyped, or moved on `D:\zt`).

### Surviving heads

| Head | Level | Parts | Main line |
|---|---|---|---|
| `azoulay2020age/2\|a` | `empirical` | 24 (`main: 5, side: 18, off: 1`) | `H\|a` 数据 → `H\|a\|1` 设计 → `H\|a\|2` 规格：基准 → `H\|a\|3` 第 1 章：主要估计 → `H\|a\|4` 第 2 章：识别检验 |
| `azoulay2020age/3\|a` | `empirical` | 41 (`main: 6, side: 34, off: 1`) | `H\|a` 数据 → `H\|a\|1` 设计 → `H\|a\|2` 规格：基准（表 1 第 1 列） → `H\|a\|3` 第 1 章：地区与事前技术均值 → `H\|a\|4` 第 2 章：事后上尾部成功均值 → `H\|a\|5` 第 3 章：识别检验与稳健性 |
| `azoulay2020age/5\|a` | `empirical` | 24 (`main: 7, side: 16, off: 1`) | `H\|a` 数据 → `H\|a\|1` 假说：条件成功概率与年龄 → `H\|a\|2` 设计 → `H\|a\|3` 规格：基准（成功退出） → `H\|a\|4` 第 0 章：无条件年龄分布 → `H\|a\|5` 第 1 章：条件极端成功概率 → `H\|a\|6` 第 2 章：识别检验与机制对照 |

### Gate

- `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs azoulay2020age`: 3 Model Cards (`2|a`, `3|a`, `5|a`), 3 candidate groups, matching the decided surviving heads.
- `zt-check --model 'azoulay2020age/2|a' --stage fold`: exit 0 (`members 24`, `loose 1`, `self_sufficient true`, `evidence_bound true`, `stage(fold) pass`).
- `zt-check --model 'azoulay2020age/3|a' --stage fold`: exit 0 (`members 41`, `loose 1`, `self_sufficient true`, `evidence_bound true`, `stage(fold) pass`).
- `zt-check --model 'azoulay2020age/5|a' --stage fold`: exit 0 (`members 24`, `loose 1`, `self_sufficient true`, `evidence_bound true`, `stage(fold) pass`).
- `list_broken_links` (`workspace: agent`): 0 broken links overall, 0 for `azoulay2020age/`.
- Tree card count: 216 Cards (unchanged).
- Term relink: not required (0 Cards edited/folded).

### Next stages

All three surviving heads (`azoulay2020age/2|a`, `azoulay2020age/3|a`, `azoulay2020age/5|a`) are `empirical` models and proceed to Stage 2 (`zt-model-settings`), Stage 3 (`zt-model-restructure`), Stage 4 (`zt-model-import`, where `3|a` and `5|a` import `2|a`'s shared administrative data/measurement blocks), and Stage 6 (`zt-evidence-dialogue`), followed by Stage 7 (`zt-literature-organize`). Stage 5 (`zt-model-illustrate`) does not apply (no formal models).

## Stage 2: settings

Date: 2026-11-27 (UTC 2026-11-28), live box `D:\zt`.

Pre-Stage-2 backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`.

### Baseline audit and per-model changes

All writes executed via `zt-digest call -Allow` serialized with `zt_mutex.py`. Formula and term-macro preservation verified programmatically before write (0 display formulas altered or dropped across all 5 edited Cards; 0 Cards moved; 0 Cards added or deleted).

1. **`azoulay2020age/2|a` (empirical, 24 parts)**
   - Baseline faults: 4 `start` faults (`2|a|a|b`, `2|a|a|c`, `2|a|a|d`, `2|a|a|e` lacked `**起点**`).
   - Edits (4 Cards): added a one-line summary, `**起点**` (`- 依赖 [[azoulay2020age/2|a|a]]：…`), and `**内容**` heading to `azoulay2020age/2|a|a|b`, `2|a|a|c`, `2|a|a|d`, and `2|a|a|e`, preserving all existing prose and `[[label@address]]` term macros verbatim.
2. **`azoulay2020age/3|a` (empirical, 41 parts)**
   - Baseline faults: 1 `unused` fault (`azoulay2020age/3|a|a|a` listed `$S$`, which appears only inside summation subscripts `\sum_{i \in S_{g,k}}` / `\sum_{i \in S_{k,m}}`).
   - Edits (1 Card): dropped the unused `$S$` row from `azoulay2020age/3|a|a|a` (`变量表`).
3. **`azoulay2020age/5|a` (empirical, 24 parts)**
   - Baseline faults: none (already clean).
   - Edits: 0 Cards; re-verified after term relink.

### Term relink

- `zt-digest term-link -Step relink -CitationKey azoulay2020age` (via `zt_mutex.py`): `{"ok":true,"complete":true,"book":"azoulay2020age","cards_changed":1,"links_added":1,"links_repaired":0,"cards_refused":0,"rounds":1,"applied":true}`.
- `zt-digest term-link -Step relink -CitationKey azoulay2020age -PlanOnly`: `{"ok":true,"complete":true,"book":"azoulay2020age","cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"rounds":0,"applied":false}`.

### Gate (post term-relink)

- `zt-check --model 'azoulay2020age/2|a' --settings`: exit 0 (`data azoulay2020age/2|a|a  variables azoulay2020age/2|a|a|a`; `settings: data tree and specification ladder are clean`).
- `zt-check --model 'azoulay2020age/2|a' --stage settings`: exit 0 (`members 24 {"main":5,"off":1,"side":18}`, `relations {"corresponds":5,"refers":20,"rests-on":15}; loose 1; self_sufficient true; evidence_bound true; stage(settings) pass`).
- `zt-check --model 'azoulay2020age/3|a' --settings`: exit 0 (`data azoulay2020age/3|a|a  variables azoulay2020age/3|a|a|a`; `settings: data tree and specification ladder are clean`).
- `zt-check --model 'azoulay2020age/3|a' --stage settings`: exit 0 (`members 41 {"main":6,"off":1,"side":34}`, `relations {"refers":84,"rests-on":12}; loose 1; self_sufficient true; evidence_bound true; stage(settings) pass`).
- `zt-check --model 'azoulay2020age/5|a' --settings`: exit 0 (`data azoulay2020age/5|a|a  variables azoulay2020age/5|a|a|a`; `settings: data tree and specification ladder are clean`).
- `zt-check --model 'azoulay2020age/5|a' --stage settings`: exit 0 (`members 24 {"main":7,"off":1,"side":16}`, `relations {"corresponds":2,"refers":30,"rests-on":15}; loose 1; self_sufficient true; evidence_bound true; stage(settings) pass`).
- `list_broken_links` (`workspace: agent`): 0 broken links overall, 0 for `azoulay2020age/`.

## Stage 3: restructure

Date: 2026-11-27 (UTC 2026-11-28), live box `D:\zt`.

Pre-Stage-3 backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`.

### Baseline audit and per-model changes

All 12 Card edits executed via `zt-digest call -Allow` serialized with `zt_mutex.py`. Zero Cards moved, added, or deleted; all display formulas (`$$...$$`) and empirical estimate numbers verified unchanged programmatically before write.

1. **`azoulay2020age/2|a` (empirical, 24 parts; 3 Cards edited)**
   - Baseline faults: 2 `lead` faults (`2|a|a|1`, `2|a|a|2` lacked `**直觉**`); `loose 1` (`2|a|b`).
   - Edits: prepended `**直觉**` leads (>=20 chars) to `2|a|a|1` (设计) and `2|a|a|2` (规格); appended plain reference `见设计 [[azoulay2020age/2|a|a|1]]。` to `2|a|b` (局限) so `loose: 1 → 0`.
2. **`azoulay2020age/3|a` (empirical, 41 parts; 4 Cards edited)**
   - Baseline faults: 3 `lead` faults (`3|a|a`, `3|a|a|1`, `3|a|a|2` lacked `**直觉**`); `loose 1` (`3|a|b`).
   - Edits: prepended `**直觉**` leads (>=20 chars) to `3|a|a` (数据), `3|a|a|1` (设计), and `3|a|a|2` (规格); appended plain reference `见设计 [[azoulay2020age/3|a|a|1]]。` to `3|a|b` (局限) so `loose: 1 → 0`.
3. **`azoulay2020age/5|a` (empirical, 24 parts; 5 Cards edited)**
   - Baseline faults: 4 `lead` faults (`5|a|a`, `5|a|a|1`, `5|a|a|2`, `5|a|a|3` lacked `**直觉**`); 1 `R22` bare LaTeX fault on `5|a|a|3` (`D_{ia}` outside `$...$`); `loose 1` (`5|a|b`).
   - Edits: prepended `**直觉**` leads (>=20 chars) to `5|a|a` (数据), `5|a|a|1` (假说), `5|a|a|2` (设计), and `5|a|a|3` (规格); wrapped `$Exit_i$`, `$D_{ia}$`, and `$a=21,\dots,65$` in `5|a|a|3`'s `**各项**` (display formula untouched); inserted plain reference `见设计 [[azoulay2020age/5|a|a|2]]。` before `来源：` on `5|a|b` (局限) so `loose: 1 → 0`.

### Term relink

- `zt-digest term-link -Step relink -CitationKey azoulay2020age` (via `zt_mutex.py`): `{"ok":true,"complete":true,"book":"azoulay2020age","cards_changed":4,"links_added":5,"links_repaired":0,"cards_refused":0,"rounds":1,"applied":true}`.
- `zt-digest term-link -Step relink -CitationKey azoulay2020age -PlanOnly`: `{"ok":true,"complete":true,"book":"azoulay2020age","cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"rounds":0,"applied":false}`.

### Gate (post term-relink)

| Model | members (main/side/off) | loose | `--stage restructure` | `--network` | `--settings` | `--skip-shared` | `--premises` |
|---|---|---|---|---|---|---|---|
| `azoulay2020age/2\|a` | 24 (5/18/1) | 0 | pass (rc=0) | clean (6 blocks, 4 layers, rc=0) | clean (rc=0) | faults: 0 (rc=0) | none (rc=0) |
| `azoulay2020age/3\|a` | 41 (6/34/1) | 0 | pass (rc=0) | clean (7 blocks, 5 layers, rc=0) | clean (rc=0) | faults: 0 (rc=0) | none (rc=0) |
| `azoulay2020age/5\|a` | 24 (7/16/1) | 0 | pass (rc=0) | clean (8 blocks, 5 layers, rc=0) | clean (rc=0) | faults: 0 (rc=0) | none (rc=0) |

- `list_broken_links` (`workspace: agent`): `{"broken":[]}` (0 broken links overall, 0 for `azoulay2020age/`).
- Advisory `R20` lines only on chapter `**本章估计**` headings (0 blocking faults).
- Tree unlocked throughout; no `card_locked` met.
