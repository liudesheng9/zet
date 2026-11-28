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
