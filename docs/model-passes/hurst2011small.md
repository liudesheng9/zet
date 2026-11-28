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
