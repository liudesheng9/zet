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
