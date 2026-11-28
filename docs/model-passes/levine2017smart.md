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
