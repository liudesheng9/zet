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

## Stage 2: settings (2026-11-27)

- Live box: `D:\zt` (`stuart2006scientists`, 270 -> 264 Cards; all writes serialized via `zt_mutex.py`, display formulas verified byte-for-byte before and after: 18/18 exact match)
- Pre-Stage-2 backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`
- Scope: empirical model `stuart2006scientists/5|a` (77 -> 71 parts); conceptual model `stuart2006scientists/3|a` skipped per `zt-post-digest`.

### Changes on `stuart2006scientists/5|a`

1. **Atomic dissolution of folded chapter `5|a|a|8` (`apply_batch`, 15 items, `links_that_will_break: []`, `link_macros_rewritten: 57`)**:
   - Moved distinct cards out before deleting duplicates:
     - Two measurement cards `5|a|a|8|a|b` and `5|a|a|8|a|c` -> `5|a|a|c` and `5|a|a|d` under unified data card `5|a|a`.
     - Four distinct identifying assumptions `5|a|a|8|a|2|a`, `2|b`, `2|d`, `2|f` -> `5|a|a|2|d`–`2|g` under unified design card `5|a|a|2` (renumbered `假设 4`–`假设 7`).
     - Barlow general specification ladder `5|a|a|8|a|3` (with its 3 variants `3|a`–`3|c`) -> `5|a|a|3|g` (and `3|g|a`–`3|g|c`) under baseline specification `5|a|a|3` (this also resolved the prior Stage 3 `forward` notice on `5|a|a|8`).
     - Calibration & sensitivity chapter `5|a|a|8|a|4` (with `4|a`–`4|d`) -> `5|a|a|8|a` (and `8|a|a`–`8|a|d`), reordered ahead of limitation card `5|a|a|8|b`.
   - Deleted 6 redundant duplicates leaf-first `into` surviving cards (each with 0 remaining successors): `5|a|a|8|a|2|c` -> `5|a|a|2|c`; `5|a|a|8|a|2|e` -> `5|a|a|2|a`; `5|a|a|8|a|2` -> `5|a|a|2`; `5|a|a|8|a|1` -> `5|a|a|1`; `5|a|a|8|a|a` -> `5|a|a|a`; `5|a|a|8|a` -> `5|a|a`.
2. **Unified data tree, design, and specification ladder (`edit_card` batch, 32 cards, 0 errors)**:
   - Merged sample/source/rule details from `8|a` into `5|a|a` (`数据`), 17 new symbol rows and dual readings from `8|a|a` into `5|a|a|a` (`变量表`, 0 `uncovered` / 0 `unused`), framing into `5|a|a|1` (`假说`), risk-set rules/threats and all 7 assumptions (`2|a`–`2|g`) into `5|a|a|2` (`设计`), and duplicate assumption details into `5|a|a|2|a` (`假设 1`) and `5|a|a|2|c` (`假设 3`).
   - Added one-line summary, `**起点**`, and `**内容**` to all 5 data-tree / descriptive cards with `start` faults (`5|a|a|b`, `5|a|a|c`, `5|a|a|d`, `5|a|a|4|a`, `5|a|a|4|b`), preserving equations `(5)` and `(6)` byte-for-byte.
   - Renamed `5|a|a|3|g` facet to `规格：Barlow 伪似然通式` (no second `规格：基准`), bound it to `5|a|a|3` with `依赖 [[stuart2006scientists/5|a|a|3]]`, and added `**相对基准的改动**`.
   - Removed redundant `（补：书中未写）` from `**估计式**` intro lines on 13 `补写` specification cards (`5|a|a|3`, `3|a`–`3|f`, `3|f|a`, `3|f|a|a`, `3|f|a|a|a`, `3|f|a|b`, `3|g|b`, `3|g|c`), keeping all equations byte-for-byte identical.
   - Updated prose assumption numbers in the 3 moved test cards (`5|a|a|8|a|b`, `8|a|c`, `8|a|d`) to `假设 5`, `假设 6`, `假设 7`.
3. **Structure & term relink**:
   - Rebuilt `5|a` `**结构**` from the 71-part build order.
   - Ran `zt-digest term-link -Step relink -CitationKey stuart2006scientists` under `zt_mutex.py` (`cards_changed: 4`, `links_added: 5`, `links_repaired: 0`, `cards_refused: 0`), confirmed with `-PlanOnly` (`cards_changed: 0`, `links_added: 0`).

### Stage 2 gate results (post term-relink)

- `zt-check --model 'stuart2006scientists/5|a' --settings`: **PASS** (exit 0; `data stuart2006scientists/5|a|a  variables stuart2006scientists/5|a|a|a`; `settings: data tree and specification ladder are clean`).
- `zt-check --model 'stuart2006scientists/5|a' --stage settings`: **PASS** (exit 0; `members 71 {"main":9,"off":1,"side":61}`, `relations {"contrasts":1,"illustrates":7,"objects":1,"refers":130,"rests-on":31}`, `loose 2`, `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`).
- `node E:\zet\skills\zt-model-settings\scripts\check-settings.mjs 'stuart2006scientists/5|a'`: **PASS** (exit 0; `settings: the data tree and the specification ladder are in place`).
- `list_broken_links` (`stuart2006scientists/`): **PASS** (`[]`, 0 broken links overall).
- Formula preservation: **PASS** (18 display formulas before and after, `exact_match: true`).
- Tree card count: **264** (270 -> 264, exact `-6` deleted redundant duplicates).

## Stage 3: restructure (2026-11-27)

- Live box: `D:\zt` (`stuart2006scientists`, 264 -> 263 Cards; all writes serialized via `zt_mutex.py`, 18 display formulas and 22 `**估计**` blocks verified byte-for-byte identical before and after: `Diffs: []`)
- Pre-Stage-3 backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`
- Scope: empirical model `stuart2006scientists/5|a` (71 -> 70 parts); conceptual model `stuart2006scientists/3|a` skipped per `zt-post-digest`.

### Changes on `stuart2006scientists/5|a`

1. **Dissolved folded inner chapter wrapper `5|a|a|8|a` into Chapter 4 (`5|a|a|8`)** (two atomic `tidy` batches):
   - Promoted the 4 calibration & sensitivity cards (`5|a|a|8|a|a`–`8|a|d`) and limitation card (`5|a|a|8|b`) to clean Side branches of `5|a|a|8` (`5|a|a|8|a`–`8|e`).
   - Deleted the redundant inner chapter card `5|a|a|8|a` `into` `5|a|a|8` (zt automatically rewrote `见 [[stuart2006scientists/5|a|a|8|a]]。` on the 4 promoted cards to `5|a|a|8`).
   - Retitled and refaceted `5|a|a|8` as `第 4 章：测量校准与敏感性核验` (`第 4 章：指数引用校准与加权、时间起点敏感性核验`, `emphasis` cleared), giving it clean empirical chapter structure (`依赖 [[stuart2006scientists/5|a|a|3|g]]`, `**直觉**`, `**本章估计**`, `**承上启下**`).
   - Preserved the cross-book link from dissolved `5|a|a|8` (`跨书关联：对照 [[babina2024entrepreneurial/4|a]]…`) on head `5|a`.
2. **Main-line `**直觉**` leads, `loose` anchoring, and R-rule cleanup** (`edit_card` batch, 9 cards + head `5|a`, 0 errors):
   - Added crisp `**直觉**` leads (>= 20 chars) to `5|a|a` (`数据`), `5|a|a|1` (`假说`), `5|a|a|2` (`设计`), `5|a|a|3` (`规格：基准`), and `5|a|a|8` (`第 4 章`), clearing all 5 `--network` `lead` faults.
   - Anchored both loose limitation cards (`5|a|a|8|e` -> `见第 4 章 [[stuart2006scientists/5|a|a|8]] 与设计 [[stuart2006scientists/5|a|a|2]]。`; `5|a|b` -> `见第 3 章 [[stuart2006scientists/5|a|a|7]] 与设计 [[stuart2006scientists/5|a|a|2]]。`), clearing `loose` 2 -> 0.
   - Cleared all 10 blocking `--skip-shared` faults: split long/semicolon items in `5|a|a` (3 × R2, 2 × R3), tightened `5|a|a|2` prose to 19 sentences without semicolons (R3, R13), split two semicolon sentences in `5|a|a|2|c` (2 × R3), and wrapped bare LaTeX `UnivPeers_{it-1}` as `$\mathrm{UnivPeers}_{it-1}$` on `5|a|a|3|a` (R22).
3. **Head `5|a` `**分部**` & `**结构**`, and term relink**:
   - Added `**分部**` for the 5-chapter model (`第一部分 数据与识别设计` from `5|a|a`, `第二部分 描述与主回归结果` from `5|a|a|4`, `第三部分 稳健性与方法核验` from `5|a|a|7`) and rebuilt `**结构**` for all 70 parts.
   - Ran `zt-digest term-link -Step relink -CitationKey stuart2006scientists` under `zt_mutex.py` (`cards_changed: 1`, `links_added: 4`, `links_repaired: 0`, `cards_refused: 0`), confirmed with `-PlanOnly` (`cards_changed: 0`, `links_added: 0`).

### Stage 3 gate results (post term-relink)

| Gate | Command | Result | Exit |
| --- | --- | --- | ---: |
| Restructure | `zt-check --model 'stuart2006scientists/5\|a' --stage restructure` | `members 70 (main:9, side:60, off:1)`, `loose 0`, `self_sufficient true`, `evidence_bound true`, `stage(restructure) pass` | 0 |
| Network | `zt-check --model 'stuart2006scientists/5\|a' --network` | `network: clean` (10 blocks, 5 layers, 414px wide, 19 paths) | 0 |
| Settings | `zt-check --model 'stuart2006scientists/5\|a' --settings` | `settings: data tree and specification ladder are clean` | 0 |
| Card rules | `zt-check --model 'stuart2006scientists/5\|a' --skip-shared` | `faults: 0` (advisory R20 only) | 0 |
| Premises | `zt-check --model 'stuart2006scientists/5\|a' --premises` | `premises: none used before their card` | 0 |
| Broken links | `list_broken_links` (`stuart2006scientists/`) | `[]` (0 broken links) | 0 |
| Integrity | Formula & estimate snapshot diff + tree count | 18 formulas & 22 `**估计**` blocks byte-for-byte unchanged (`Diffs: []`); tree 264 -> 263 (`-1` dissolved inner chapter) | 0 |

## Stage 4: imports and explanations (2026-11-27)

- Live box: `D:\zt` (`stuart2006scientists`, 263 Cards unchanged; all writes serialized via `zt_mutex.py`)
- Pre-Stage-4 backup: `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3`
- Scope: conceptual lender `stuart2006scientists/3|a` (4 parts) and empirical model `stuart2006scientists/5|a` (70 -> 71 members with 1 shared `单卡` import).

### Changes

1. **Empirical hypothesis wired to conceptual predictions (`5|a|a|1` -> `3|a|a|2`)**:
   - Replaced `参照 [[stuart2006scientists/3|a]]：本假说来自它的预测。` on `5|a|a|1` (`假说：检验工作情境、博士烙印与合著网络的八项假说`) with `依赖 [[stuart2006scientists/3|a|a|2]]：本假说逐项检验概念模型导出的八项工作情境、博士烙印与合著网络假说（经 **引入**）。`.
   - Ran `imports.mjs plan` (`premise: 单卡 [[stuart2006scientists/3|a|a|2]]`) and `imports.mjs apply` under `zt_mutex.py` (`wrote 1 import line(s); tolerated 0 -> 1; check passes`).
   - Kept the import `单卡` (did not apply the round-2 `upgrade: 整块` proposal): per `zt-model-import`, an empirical hypothesis takes the theory's prediction alone, not its mechanism/setting chain; `3|a|a|1` (`机制`) stays `tolerated` (1, advice only) in its home model `3|a`.
   - Refined the `**引入**` line in `5|a` to `- 单卡 [[stuart2006scientists/3|a|a|2]]：概念模型导出的假说 1 至假说 8（工作情境、博士烙印与跨校合著网络对学术创业的影响方向与调节预测）`.
   - Both `3|a` and `5|a` have `false_share: []`, 0 copies, and 0 cross-book imports (existing cross-book `对照` comparisons stay on `3|a` and `5|a` for `zt-evidence-dialogue`). `3|a` now reports `lent: [{"model":"stuart2006scientists/5|a","roots":["stuart2006scientists/3|a|a|2"]}]`.
2. **Explanation refresh (step 8)**:
   - `zt-digest model-explain -Step plan -Model 'stuart2006scientists/5|a'` returned `nothing to explain` (`parts: 0, jobs: []`) both before and after the import; no explanation job needed and no DeepSeek run started.
3. **Term relink**:
   - Ran `zt-digest term-link -Step relink -CitationKey stuart2006scientists` under `zt_mutex.py` (`cards_changed: 2`, `links_added: 2`, `links_repaired: 0`, `cards_refused: 0`), confirmed with `-PlanOnly` (`cards_changed: 0`, `links_added: 0`).

### Stage 4 final gate results (post term-relink)

| Gate | Command | Result | Exit |
| --- | --- | --- | ---: |
| Imports (`5\|a`) | `node E:\zet\skills\zt-model-import\scripts\imports.mjs check 'stuart2006scientists/5\|a'` | pass (`tolerated` 1 advice: `3\|a\|a\|2` -> `3\|a\|a\|1`) | 0 |
| Imports (`3\|a`) | `node E:\zet\skills\zt-model-import\scripts\imports.mjs check 'stuart2006scientists/3\|a'` | pass (0 faults, 0 advice) | 0 |
| Final (`5\|a`) | `zt-check --model 'stuart2006scientists/5\|a' --stage final` | `members 71 (main:9, side:60, shared:1, off:1)`, `loose 0`, `self_sufficient true`, `evidence_bound true`, `stage(final) pass` | 0 |
| Settings (`5\|a`) | `zt-check --model 'stuart2006scientists/5\|a' --settings` | `settings: data tree and specification ladder are clean` | 0 |
| Network (`5\|a`) | `zt-check --model 'stuart2006scientists/5\|a' --network` | `network: clean` (11 blocks incl. `共用` shelf, 5 layers, 428px wide, 19 paths) | 0 |
| Card rules (`5\|a`) | `zt-check --model 'stuart2006scientists/5\|a' --skip-shared` | `faults: 0` (advisory R20 only) | 0 |
| Final (`3\|a`) | `zt-check --model 'stuart2006scientists/3\|a' --stage final` | `members 4 (main:3, side:1)`, `loose 0`, `self_sufficient true`, `stage(final) pass` | 0 |
| Broken links | `list_broken_links` (`stuart2006scientists/`) | `{"broken":[]}` (0 broken links) | 0 |

## Stage 5: evidence dialogue

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage5-dialogue-2026-11-27.sqlite3`.

- `dialogue.mjs survey 'stuart2006scientists/5|a'`: `ours: 22`, `theirs: 2701`, `pairs: 0` (0 candidate pairs; dialogue complete with no edits needed).
- Gate: `dialogue.mjs check` not needed / clean (`0` pairs proposed).

## Stage 6: literature organize (2026-11-28)

- Live box: `D:\zt` (`stuart2006scientists`, 263 Cards unchanged; all writes serialized via `zt_mutex.py`)
- Pre-Stage-6 backup: `D:\zt\backups\zt-before-iof10-stage6-organize-2026-11-27.sqlite3`
- Method: four atomic `batch.mjs` moves (tail-to-head), one `leads.mjs` run (`3` rewrites), scripted `edit_card` updates (root index, structure Card `1|d`, and moved crosser `3|e`), followed by `zt-digest term-link -Step relink`.

### Source divisions

From `D:\zt-digest\stuart2006scientists\book.zh.md`: `# 引言`; `# 研究背景：科学规范与学术创业的演变`; `# 研究假说：学术创业的社会与组织影响因素` (`## 工作情境对向创业型科学转型的影响`, `## 烙印效应与科学规范的维系`, `## 社会网络对向创业型科学转型的影响`, `## 关于社会机制的注意事项`); `# 研究方法与数据` (`## 病例—队列设计`, `## 统计方法`, `## 变量定义`); `# 实证结果` (`## 稳健性检验：社会影响抑或内生匹配？`); `# 结论`; `# 附录` (`## 数据来源`); followed by the Term list and Cited list at the tail.

### Final tree (top line and divisions)

```
1  引言 (8)                  c 摘要与引言主链 (1|c..1|c|5) · d 全书结构卡 (1|d)
2  研究背景 (9)              a 科学规范与学术创业演变 (2|a..2|a|4, 含脚注与 1934 哈佛专利禁令事件)
3  研究假说 (16, rewrite)    a 概念模型 (3|a, 4 parts) · b 工作情境 (3|b..3|b|3) · c 烙印效应 (3|c)
                             d 社会网络 (3|d..3|d|1) · e 关于社会机制的注意事项 (3|e, 原 4|b)
4  研究方法与数据 (11, rewrite) a 病例—队列设计 (4|a..4|a|1) · b 统计方法 (4|b..4|b|1) · c 变量定义 (4|c..4|c|4)
5  实证结果 (99, rewrite)    a 实证模型 (5|a, 70 tree cards / 71 members) · b 主实证结果 (5|b..5|b|6)
                             c 稳健性检验：社会影响抑或内生匹配？ (5|c..5|c|2, 含 1976 基因泰克事件) · d–r 15 条实证发现
6  结论 (8)                  a 结论主链 (6|a..6|a|6)
7  附录 (7)                  a 数据来源 (7|a..7|a|5)
8  术语表 (30, from 1|b)     Term list
9  参考文献 (74, from 1|a)   Cited list
```

### Crossers and how each was settled

- **Old `1` held §引言 and both reference lists**: Term list (`1|b`) and Cited list (`1|a`) moved to the tail as `8` and `9` (692 Link macros rewritten, 0 broken); structure Card `1|d` stays under `1` since the source has no front matter.
- **Old `3|a` (conceptual Model Card) carried 9 reading notes on its Direct chain**: the 9 reading notes (`3|a|1..3|a|7`, incl. `3|a|2|a` and `3|a|6|a`) were lifted off `3|a`'s Direct chain into three `##` Side branches of `3` (`3|b` 工作情境, `3|c` 烙印效应, `3|d` 社会网络). No model part moved out (`3|a` retains all 4 model parts = 5 cards total; `zt-check --tree --snapshot` reports the expected `14 Cards before, 5 now` unit-size delta from detaching those 9 reading notes).
- **Old `4|b` (`§关于社会机制的注意事项`) was a crosser in §4**: in the source (`book.zh.md` L123), `## 关于社会机制的注意事项` is the 4th `##` subdivision of `# 研究假说`, before `# 研究方法与数据` (L129). Moved `4|b` to `3|e` as the 4th subdivision of `3`, updated its opening sentence from `作者在方法部分开篇提示如何看待社会机制。` to `作者在研究假说末尾提示如何看待社会机制。`, and split the remaining `4|b|1..4|b|9` chain into `4`'s three `##` subdivisions (`4|a` 病例—队列设计, `4|b` 统计方法, `4|c` 变量定义).
- **Old `5|b` chained main results and `## 稳健性检验`**: split `5|b|7..5|b|9` (with event `5|b|7|a` -> `5|c|a`) off `5|b` into `5`'s Side branch `5|c` (`稳健性检验：社会影响抑或内生匹配？`), ordered right after `5|a` (empirical model) and `5|b` (main results), ahead of the 15 Finding Cards (`5|d..5|r`).
- **Summary leads & indexes**: `leads.mjs` rewrote `3`, `4`, and `5` with `小节索引：`, `在全书中的位置：`, and `来源：[[stuart2006scientists]] …`. Updated root card `stuart2006scientists` to list all 9 top divisions (`1`–`9`) and updated structure Card `1|d` to remove the pre-fold duplicate model entry (`5|a|a|8`) and list the final 2 models (`3|a`, `5|a`) and 2 events (`2|a|3|a|a`, `5|c|a`).

### Stage 6 gate results (post term-relink)

| Gate | Command | Result | Exit |
| --- | --- | --- | ---: |
| Term relink | `zt-digest term-link -Step relink -CitationKey stuart2006scientists` & `-PlanOnly` | `cards_changed: 0`, `links_added: 0`, `ok: true` | 0 |
| Tree check | `zt-check --tree stuart2006scientists` | 9 top Cards (`1`–`7`, `8` Term list, `9` Cited list), `no fault` | 0 |
| Snapshot diff | `zt-check --tree stuart2006scientists --snapshot …` | 263 Cards unchanged; only expected `3|a` 14 -> 5 delta (9 reading notes detached from conceptual Model Card's Direct chain) | 1 (expected) |
| Model survey | `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs stuart2006scientists` | 2 Model Cards: `3|a` (`concept`, 4 parts), `5|a` (`empirical`, 71 members) | 0 |
| Final (`5\|a`) | `zt-check --model 'stuart2006scientists/5\|a' --stage final` | `members 71 (main:9, side:60, shared:1, off:1)`, `loose 0`, `self_sufficient true`, `evidence_bound true`, `stage(final) pass` | 0 |
| Settings (`5\|a`) | `zt-check --model 'stuart2006scientists/5\|a' --settings` | `settings: data tree and specification ladder are clean` | 0 |
| Network (`5\|a`) | `zt-check --model 'stuart2006scientists/5\|a' --network` | `network: clean` (11 blocks, 5 layers, 428px wide, 19 paths) | 0 |
| Final (`3\|a`) | `zt-check --model 'stuart2006scientists/3\|a' --stage final` | `members 4 (main:3, side:1)`, `loose 0`, `self_sufficient true`, `stage(final) pass` | 0 |
| Edited cards | `zt-check --cards "stuart2006scientists,…,5"` | `faults: 0` (advisory lines only) | 0 |
| Broken links | `list_broken_links` (`stuart2006scientists/`) | `[]` (0 broken links) | 0 |


## Stage 7: illustrate

Not applicable (`stuart2006scientists` holds 1 empirical model and 1 concept model, 0 formal models; Stage 7 applies to formal models only).
