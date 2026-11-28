# stuart2006scientists: post-digest sequence

Owner: Xingtong Chen

Stuart & Ding (2006), *When Do Scientists Become Entrepreneurs? The Social Structural Antecedents of Commercial Activity in the Academic Life Sciences* (*American Journal of Sociology*, 112(1), 97–144).

- Live box: `D:\zt` (tree `stuart2006scientists`, 270 Cards, unlocked throughout)
- Pre-Stage-1 backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3`

## Stage 1: fold (2026-11-27)

### Decision table

Initial survey found 3 Model Cards (`3|a` concept, 4 parts; `4|a` empirical, 22 parts; `5|a` empirical, 54 parts). Checked against `D:\zt-digest\stuart2006scientists\book.zh.md` (§研究假说, §研究方法与数据, §实证结果, §稳健性检验).

| Initial Model Card | Level | Initial Parts | Belongs to Model | Surviving Head | Reason |
| --- | --- | ---: | --- | --- | --- |
| `stuart2006scientists/3|a` | `concept` | 4 | Conceptual social-structure model (§研究假说) | `stuart2006scientists/3|a` | Own conceptual model (three-layer social structure generating Hypotheses 1–8); theory/concept and empirical test are never folded together. |
| `stuart2006scientists/4|a` | `empirical` | 22 | Case-cohort Cox & WESML empirical model (§研究方法与数据 + §实证结果 + §稳健性检验) | `stuart2006scientists/5|a` | Same estimand (conditional hazard rate of first transition to biotech founding/SAB) on the exact same dataset (6,037 U.S. life science PhDs = 917 IPO events + 5,120 academic subcohort, 103,572 person-years); `4|a` holds the Methods/Design/Specs/Calibration half split from Results `5|a` by digest group boundaries. |
| `stuart2006scientists/5|a` | `empirical` | 54 | Case-cohort Cox & WESML empirical model (§研究方法与数据 + §实证结果 + §稳健性检验) | `stuart2006scientists/5|a` | Surviving empirical head: already holds the fuller 54-part results & robustness tree (Tables 1–4, Fig. 1, 12 progressive Cox & WESML specs, 5 core estimates in `**主要估计**`, 3 cross-book links, and 0 non-part Idea-note descendants). |

### Fold actions (`4|a` -> `5|a`)

Executed via `fold.mjs` under `zt_mutex.py` (no math or prose changed; no Links deleted except collapsing `3|a`'s duplicate pre-fold `对应` line):

1. **`3|a` (`edit_card`)**: Collapsed the two pre-fold `对应` lines pointing to `4|a` and `5|a` into a single `- 对应 [[stuart2006scientists/5|a]]：实证模型操作化并检验全部八项工作情境与社会结构假说。` so `3|a` links only to the surviving empirical head (preventing a `reach_in` fault once `4|a` became a part of `5|a`).
2. **Strip `**结构**` on `4|a` (`edit_card`)**: Removed `4|a`'s `**结构**` block (neither `4|a` nor `5|a` had an `**引入**` section).
3. **Batch A (`tidy`, atomic)**:
   - Moved `4|a`'s 11 non-part Idea-note descendants (`4|a|1..10`, `4|a|5|a`) off `4|a` onto section anchor `4` as `4|b..4|b|9` (`4|b|4|a`).
   - Parked `4|a|a` (21 parts subtree) and `4|a|b` (1 part: limitation) as Side successors of `5|a` (`5|a|c` and `5|a|d`); zt automatically rewrote the two part Links in `4|a`'s `**主要估计**` table to the parked `5|a|c|…` addresses.
4. **Mask, retype, restore on `4|a`**:
   - Masked 2 Links targeting `stuart2006scientists/5|a` in `4|a`.
   - Retyped `4|a` alone (`facet: "设计与规格"`, `emphasis: "core"`, `level: ""`).
   - Restored `4|a`'s masked body.
5. **Batch B (`tidy`, atomic)**:
   - Moved `4|a` (`alone: true`) onto `5|a`'s main line after `5|a|a|7` as `5|a|a|8`.
   - Moved parked subtrees `5|a|c` (21 parts) and `5|a|d` (1 part) under `5|a|a|8` as `5|a|a|8|a` and `5|a|a|8|b`.
6. **Term relink**: Ran `zt-digest term-link -Step relink -CitationKey stuart2006scientists` under `zt_mutex.py` (`cards_changed: 0, links_added: 0`), confirmed with `-PlanOnly` (`0` changes).

### Surviving heads (2 models)

| Surviving Head | Level | Parts | Scope |
| --- | --- | ---: | --- |
| `stuart2006scientists/3|a` | `concept` | 4 | Three-layer social structure conceptual model (workplace colleagues, PhD socialization, cross-university coauthorship; Hypotheses 1–8) |
| `stuart2006scientists/5|a` | `empirical` | 77 | Case-cohort Barlow pseudo-likelihood Cox & Manski-Lerman WESML empirical model (6,037 scientists, 103,572 person-years, 917 events; design, variable measurement, calibration, Tables 1–4 & Fig. 1) |

### Stage 1 gate results

- `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs stuart2006scientists`: **PASS** — lists exactly 2 surviving heads (`3|a` concept 4 parts; `5|a` empirical 77 parts), 2 candidate groups (`read` kin only).
- `zt-check --model 'stuart2006scientists/3|a' --stage fold`: **PASS** (exit 0; `members 4`, `self_sufficient true`, `stage(fold) pass`).
- `zt-check --model 'stuart2006scientists/5|a' --stage fold`: **PASS** (exit 0; `members 77`, `stage(fold) pass`; 3 expected deferred notices for later stages: Stage 2 `spec_unbound` on `5|a|a|8|a|3`, Stage 3 `forward` and `unanchored` on folded former head `5|a|a|8`).
- `list_broken_links` (`stuart2006scientists/`): **PASS** (`[]`, 0 broken links).
- Tree card count: **270** (unchanged).
- `zt-digest term-link -Step relink -CitationKey stuart2006scientists` & `-PlanOnly`: **PASS** (`cards_changed: 0`, `links_added: 0`, `ok: true`).

### Next stages

- `stuart2006scientists/3|a` (concept): no further model passes needed (fold only).
- `stuart2006scientists/5|a` (empirical): hand to `zt-model-settings` (merge duplicate Data/Variable/Hypothesis/Design/Assumption/Spec subtrees from `5|a|a|8|a` into the unified setting & specification ladder), then `zt-model-restructure`, `zt-model-import` (import concept predictions from `3|a`), and `zt-evidence-dialogue`.
