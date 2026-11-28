# Model Passes: `gottlieb2022career`

Owner: Xingtong Chen

- **Citation key:** `gottlieb2022career` (*Does Career Risk Deter Potential Entrepreneurs?*, Gottlieb, Townsend & Xu 2022)
- **Archive:** Live box `D:\zt`
- **Pre-Stage-1 backup:** `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3`

## Stage 1: fold

### Decision table (11 initial Model Cards -> 6 surviving models)

Decided by model nature per `zt-model-fold` and `docs/empirical-models-proposal.md` §9.1 (same estimand on same data + its measurement, validity, robustness, heterogeneity, and mechanism decompositions belong to one empirical model; different dataset or different estimand is its own empirical model; concept/theory models never fold with empirical tests):

| Initial Model Card | Level | Initial Parts | Model Group | Surviving Head | Decision & One-line Reason |
| --- | --- | ---: | --- | --- | --- |
| `gottlieb2022career/2\|a` | empirical | 23 | 2006 Census Double-RD | `gottlieb2022career/4\|a` | Fold into `4\|a`: data construction, birth-date/entrepreneurship measurement, and Table 1 descriptive baseline built directly for the 2006 Census Double-RD sample ($N=86{,}565$). |
| `gottlieb2022career/4\|a` | empirical | 21 | 2006 Census Double-RD | `gottlieb2022career/4\|a` | Surviving head: core 2006 Census Double-RD identification design, Eqs. (1)–(3), first-stage leave verification (Fig. 1), and LFS co-movement check (§3). |
| `gottlieb2022career/5\|a` | concept | 3 | Counterfactual Paths | `gottlieb2022career/5\|a` | Own model: conceptual 3-state labor-market preference-ordering model of counterfactual transition paths (§4.2); never folded with empirical tests and distinct from `8\|a`. |
| `gottlieb2022career/6\|a` | empirical | 45 | 2006 Census Double-RD | `gottlieb2022career/4\|a` | Fold into `4\|a`: exact same 2006 Census Double-RD estimand and Eq. (3) — validity tests (Table 3, Fig. 2), baseline estimates (Table 4, Fig. 3), placebos (Table 5), and robustness (Tables 6–7, §5.1–§5.3). |
| `gottlieb2022career/7\|a` | empirical | 18 | 2006 Census Double-RD | `gottlieb2022career/4\|a` | Fold into `4\|a`: provincial $\Delta\text{Weeks Leave}$ heterogeneity interaction of Eq. (3) on the exact same 2006 Census sample ($N=86{,}565\text{–}42{,}430$, Table 8, §5.4). |
| `gottlieb2022career/7\|b` | empirical | 31 | 2006 Census Double-RD | `gottlieb2022career/4\|a` | Fold into `4\|a`: entrepreneurship quality decomposition (incorporated, employees, high hours/income) using Eq. (3) on the exact same 2006 Census 30-day sample ($N=42{,}430$, Table 9, §5.5). |
| `gottlieb2022career/7\|c` | empirical | 31 | LAD Short-Run & Dynamics | `gottlieb2022career/7\|c` | Own model: independent Longitudinal Administrative Databank (LAD) annual tax panel of pre-birth pure wage-earners ($N=27{,}300$ at 30d) with loose net self-employment measure for 2001–2005 dynamics & EI check (Table 10, §5.7). |
| `gottlieb2022career/8\|a` | concept | 4 | Direct vs Indirect Mechanism | `gottlieb2022career/8\|a` | Own model: conceptual framework contrasting direct experimentation vs indirect preference-shift mechanisms and their four comparative-static predictions (§6); never folded with empirical tests. |
| `gottlieb2022career/8\|b` | empirical | 39 | Indirect Mechanism Falsification | `gottlieb2022career/8\|b` | Own model: distinct estimand (non-entrepreneurship industry, P75 wage, and geographic mobility) on wage-stayers in LAD ($N=22{,}725\text{–}18{,}375$) and 2006 Census migration sample ($N=22{,}960$, Table 11, §6.1). |
| `gottlieb2022career/8\|c` | empirical | 21 | 2006 Census Double-RD | `gottlieb2022career/4\|a` | Fold into `4\|a`: direct-mechanism industry decomposition (SBO startup capital & Orbis 5-year failure rate) repeating Eq. (3) on the exact same 2006 Census 30-day sample ($N=42{,}430$, Table 12, §6.3). |
| `gottlieb2022career/8\|d` | empirical | 31 | Career-Cost Heterogeneity | `gottlieb2022career/8\|d` | Own model: cohesive career-cost-of-failure heterogeneity test spanning LAD 4-year industry wage-penalty interactions ($N=27{,}300$, 2002–2005) and 2006 Census college-degree subsamples ($N=11{,}835 / 30{,}595$, Table 13, §6.3). |

### Fold actions executed

All 5 folds into `gottlieb2022career/4|a` were executed in book order (`2|a`, `6|a`, `7|a`, `7|b`, `8|c`) under `zt_mutex.py` without altering mathematics or deleting any Links:

1. **`2|a` -> `4|a|a|4`** (`facet: "第 1 节：数据与创业测量"`, `emphasis: ""`, `level: ""`): stripped `**结构**`, moved Idea-note chain `2|a|1..4` (6 cards) to section anchor `2` (`2|d..2|d|3`), parked `2|a|a..b` on `4|a`, masked 6 part Links, retyped, restored body, placed `2|a` alone at `4|a|a|4` with parts restored at `4|a|a|4|a..b`.
2. **`6|a` -> `4|a|a|5`** (`facet: "第 5.1–5.3 节：基准估计与稳健性"`, `emphasis: "core"`, `level: ""`): stripped `**结构**`, moved Idea-note chain `6|a|1..9` (9 cards) to section anchor `6`, parked `6|a|a..b` on `4|a`, masked 8 part Links in `6|a` and 1 Link in `5|a`, retyped, restored bodies of `6|a` and `5|a`, placed `6|a` alone at `4|a|a|5` with parts restored at `4|a|a|5|a..b`.
3. **`7|a` -> `4|a|a|6`** (`facet: "第 5.4 节：省际异质性"`, `emphasis: ""`, `level: ""`): stripped `**结构**`, parked `7|a|a..b` on `4|a`, masked 4 part Links in `7|a` and 1 Link in `5|a`, retyped, restored bodies of `7|a` and `5|a`, placed `7|a` alone at `4|a|a|6` with parts restored at `4|a|a|6|a..b`.
4. **`7|b` -> `4|a|a|7`** (`facet: "第 5.5 节：创业企业质量"`, `emphasis: ""`, `level: ""`): stripped `**结构**`, parked `7|b|a..c` on `4|a`, masked 10 part Links in `7|b`, retyped, restored body, placed `7|b` alone at `4|a|a|7` with parts restored at `4|a|a|7|a..c`.
5. **`8|c` -> `4|a|a|8`** (`facet: "第 6.3 节：行业启动资金与失败率"`, `emphasis: ""`, `level: ""`): stripped `**结构**`, parked `8|c|a..b` on `4|a`, masked 4 part Links in `8|c` and 1 Link in `8|a`, retyped, restored bodies of `8|c` and `8|a`, placed `8|c` alone at `4|a|a|8` with parts restored at `4|a|a|8|a..b`.

### Surviving heads (6)

| Address | Level | Parts | Summary |
| --- | --- | ---: | --- |
| `gottlieb2022career/4\|a` | empirical | 164 | 2006 Canadian Census Double-RD entrepreneurship model: data & measurement (§1), design & Eqs. (1)–(3) (§3), validity/baseline/placebo/robustness (§5.1–§5.3), provincial heterogeneity (§5.4), quality decomposition (§5.5), and industry startup-capital/failure-rate decomposition (§6.3, Table 12). |
| `gottlieb2022career/5\|a` | concept | 5 (3 own + 2 false_share) | Counterfactual labor-market transition paths model (§4.2); 2 `false_share` links to folded chapters `4\|a\|a\|5` and `4\|a\|a\|6` deferred to Stage 4 (`zt-model-import`). |
| `gottlieb2022career/7\|c` | empirical | 31 | LAD longitudinal tax panel short-run (2002) & dynamic (2001–2005) Double-RD + EI sanity check (§5.7, Table 10). |
| `gottlieb2022career/8\|a` | concept | 5 (4 own + 1 false_share) | Direct vs indirect behavioral mechanism conceptual framework (§6); 1 `false_share` link to folded chapter `4\|a\|a\|8` deferred to Stage 4 (`zt-model-import`). |
| `gottlieb2022career/8\|b` | empirical | 39 | Indirect mechanism falsification: industry, P75 wage, and geographic mobility among wage-stayers (§6.1, Table 11). |
| `gottlieb2022career/8\|d` | empirical | 31 | Direct mechanism test II: LAD industry entrepreneurial wage-penalty interactions & 2006 Census college-degree heterogeneity (§6.3, Table 13). |

### Stage 1 Gate verification

- **`model-survey.mjs gottlieb2022career`:** PASS — lists exactly the 6 surviving heads (`4|a`, `5|a`, `7|c`, `8|a`, `8|b`, `8|d`), 6 candidate groups, 0 structural joins.
- **`zt-check --model '<head>' --stage fold`:** PASS (exit 0 on all 6 heads):
  - `4|a`: `members 164 {"main":9,"off":1,"side":154}`; `stage(fold) pass` (only expected deferred notices: 5 Stage 3 `unanchored` on folded chapters `4|a|a|4..8` and 5 Stage 2 `spec_unbound` on their baseline specs).
  - `5|a`: `members 5 {"main":3,"shared":2}`; `self_sufficient true; stage(fold) pass`.
  - `7|c`: `members 31 {"main":6,"off":1,"side":24}`; `self_sufficient true; evidence_bound true; stage(fold) pass`.
  - `8|a`: `members 5 {"main":3,"off":1,"shared":1}`; `self_sufficient true; stage(fold) pass`.
  - `8|b`: `members 39 {"main":7,"off":1,"side":31}`; `self_sufficient true; evidence_bound true; stage(fold) pass`.
  - `8|d`: `members 31 {"main":7,"off":1,"side":23}`; `self_sufficient true; evidence_bound true; stage(fold) pass`.
- **`list_broken_links`:** PASS — 0 broken links in `gottlieb2022career/`. Tree card count: **489** (unchanged).
- **`zt-digest term-link -Step relink -CitationKey gottlieb2022career` (and `-PlanOnly`):** PASS — `ok: true, complete: true, cards_changed: 0, links_added: 0, links_repaired: 0, cards_refused: 0`.
- **Handoff:** Next stage is `zt-model-settings` (Stage 2), then `zt-model-restructure` (Stage 3), then `zt-model-import` (Stage 4) on the 6 surviving heads (`4|a`, `5|a`, `7|c`, `8|a`, `8|b`, `8|d`).

## Stage 2: settings

- **Date:** 2026-11-27
- **Archive:** Live box `D:\zt` (all writes serialized via `zt_mutex.py`)
- **Pre-Stage-2 backup:** `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`
- **Scope:** Strict `zt-model-settings` on the 4 empirical models (`4|a`, `7|c`, `8|b`, `8|d`); concept models `5|a` and `8|a` skipped per `zt-post-digest`. Every display formula and equation tag was verified byte-for-byte before every write.

### Per-model changes

1. **`gottlieb2022career/4|a` (empirical, 164 → 146 parts):**
   - **Atomic dissolution (`apply_batch`, 38 items, 114 moved, 18 deleted `into` surviving cards, `links_that_will_break: []`):**
     - Moved 4 measurement cards (`4|a|b`, `4|a|c`, `7|a|b`, `8|a|b`) to `4|a|a|b–e` under the unified data card `4|a|a`.
     - Moved 11 distinct identifying assumptions (`4|a|1|a–d`, `5|a|1|a–d`, `6|a|2|b`, `7|a|1|b`, `8|a|2|b`) to `4|a|a|1|f–p` under the unified design card `4|a|a|1` (renumbered `假设 5`–`假设 15`).
     - Moved the 5 folded specification ladders (`4|a|2`, `5|a|2`, `6|a|3`, `7|a|2`, `8|a|3`, with all Side variants intact) to `4|a|a|2|e–i` under `规格：基准` (`4|a|a|2`).
     - Deleted 18 redundant duplicates `into` their surviving counterparts: 3 "inherits base DD-RD" assumptions (`6|a|2|a`, `7|a|1|a`, `8|a|2|a` → `4|a|a|1|c`), 5 duplicate `变量表` (`→ 4|a|a|a`), 5 duplicate `设计` (`→ 4|a|a|1`), and 5 duplicate `数据` (`→ 4|a|a`). Folded sub-chapters and hypotheses rose cleanly into `4|a|a|4|a..8|a` ahead of their `局限` cards.
   - **Unified text edits (45 cards + head `**结构**`):**
     - Merged unique sample/source/design/threat details into `4|a|a` (`数据`) and `4|a|a|1` (`设计`, listing `假设 1`–`假设 15`).
     - Unified `4|a|a|a` (`变量表`, 30 rows covering all symbols across Eqs. (1)–(4), Table 1, Tables 3–9, and Table 12; 0 `uncovered`, 0 `unused`).
     - Added one-line summary + `**起点**` + `**内容**` to `4|a|a|b–e` (fixed 4 `start` faults).
     - Re-faceted `4|a|a|2|e–i` to non-baseline facets (`规格：母亲描述统计（表 1）`, `规格：30 天带宽基准（表 4 第 4 列）`, `规格：省际全交互（表 8 第 4 列）`, `规格：自报总创业率（表 9 第 1 列）`, `规格：高启动资金行业（表 12 第 1 列）`), added `依赖 [[gottlieb2022career/4|a|a|2]]` and `**相对基准的改动**` (fixed 5 `variant_change` + 5 `spec_unbound` faults).
     - Removed redundant `（补：书中未写）` marker from 26 `补写` specification cards while keeping canonical `原书没有写出本规格的估计式。本卡按……补写：` and all display formulas intact (fixed 26 `equation` faults).
     - Rebuilt `**结构**` on `gottlieb2022career/4|a` (146 entries in `build.order`).
2. **`gottlieb2022career/7|c` (empirical, 31 parts, 0 moves):**
   - Added one-line summary + `**起点**` + `**内容**` to `7|c|a|b` (`start` fault); removed redundant `（补：书中未写）` from 8 `补写` spec cards `7|c|a|2|a–h` (`equation` faults).
3. **`gottlieb2022career/8|b` (empirical, 39 parts, 0 moves):**
   - Added `**起点**` + `**内容**` to `8|b|a|b` (`start` fault); removed redundant `（补：书中未写）` from 12 `补写` spec cards `8|b|a|3` and `8|b|a|3|a–k` (`equation` faults).
4. **`gottlieb2022career/8|d` (empirical, 31 parts, 0 moves):**
   - Added one-line summary + `**起点**` + `**内容**` to `8|d|a|b` (`start` fault); removed redundant `（补：书中未写）` from 6 `补写` spec cards `8|d|a|3` and `8|d|a|3|a–e` (`equation` faults).

### Stage 2 Gate verification (post term-relink)

- **Term relink (`zt-digest term-link -Step relink -CitationKey gottlieb2022career`):** `ok: true, complete: true, cards_changed: 8, links_added: 18, links_repaired: 0, cards_refused: 0`; `-PlanOnly` confirmed `cards_changed: 0, links_added: 0`.
- **`zt-check --model '<head>' --settings` & `check-settings.mjs '<head>' --symbols`:** PASS (exit 0 on all 4 empirical models — `data tree and specification ladder are clean` / `the data tree and the specification ladder are in place`).
- **`zt-check --model '<head>' --stage settings`:** PASS (exit 0 on all 4 empirical models):
  - `4|a`: `members 146 {"main":9,"off":1,"side":136}`, `relations {"corresponds":4,"illustrates":4,"refers":265,"rests-on":63}`, `evidence_bound true`, `stage(settings) pass` (only expected Stage 3 deferred `unanchored` notices on folded chapters `4|a|a|4..8`).
  - `7|c`: `members 31 {"main":6,"off":1,"side":24}`, `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`.
  - `8|b`: `members 39 {"main":7,"off":1,"side":31}`, `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`.
  - `8|d`: `members 31 {"main":7,"off":1,"side":23}`, `self_sufficient true`, `evidence_bound true`, `stage(settings) pass`.
- **`list_broken_links`:** PASS — 0 broken links total, 0 for `gottlieb2022career/`. Tree card count: **489 → 471** (−18 deleted duplicates).
- **Handoff:** Ready for Stage 3 (`zt-model-restructure`).

## Stage 3: restructure

- **Date:** 2026-11-27
- **Archive:** Live box `D:\zt` (all writes serialized via `zt_mutex.py`)
- **Pre-Stage-3 backup:** `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`
- **Scope:** Full `zt-model-restructure` on the 4 empirical models (`4|a`, `7|c`, `8|b`, `8|d`); concept models `5|a` and `8|a` skipped. All unique display formulas (`\tag{1}`–`\tag{4}` and every specification formula) and all empirical estimate numbers were verified byte-for-byte unchanged.

### Per-model changes

1. **`gottlieb2022career/4|a` (empirical, 146 → 138 parts):**
   - **Atomic structural reorganization (`apply_batch`, 77 moved, 8 redundant inner chapter wrappers deleted `into` surviving chapter heads, `links_that_will_break: []`):**
     - Dissolved inner chapter `4|a|a|4|a` `into` `4|a|a|4` and moved `4|a|a|4` before `4|a|a|3` as `第 1 章：描述性统计基线` (§1, Table 1); `4|a|a|3` became `第 2 章：设计前提验证` (§3, Fig. 1 & LFS).
     - Promoted `4|a|a|5|a` (§5.1) before `4|a|a|5` as `第 3 章：识别有效性检验`, promoted `4|a|a|5|a|2` (§5.3) after `4|a|a|5` as `第 5 章：稳健性检验`, and dissolved `4|a|a|5|a|1` (§5.2) `into` `4|a|a|5` (`4|a|a|6`, `第 4 章：基准估计与安慰剂`; `4|a|a|5`'s duplicate display formulas already live byte-for-byte on `4|a|a|2|f` and `4|a|a|2|f|d`).
     - Dissolved inner chapter `4|a|a|6|a|1` `into` `4|a|a|8` (`第 6 章：省际异质性`, §5.4); dissolved `4|a|a|7|a` & `4|a|a|7|a|1` `into` `4|a|a|9` (`第 7 章：创业企业质量分解`, §5.5, preserving cross-book link `guzman2020state/3|a`); dissolved `4|a|a|8|a|1..3` `into` `4|a|a|10` (`第 8 章：行业启动资金与失败率`, §6.3).
   - **Card & head edits (20 cards):**
     - Rewrote `4|a|a|3`–`4|a|a|10` with clean `第 1 章`–`第 8 章` titles/facets (`emphasis: core`), `**直觉**` (>=20 chars), `**本章估计**`, `**承上启下**`, and explicit `依赖` anchors (`unanchored` 5 → 0).
     - Added crisp `**直觉**` leads to `4|a|a|1` (`设计`) and `4|a|a|2` (`规格：基准`); cleared all 21 baseline `--skip-shared` faults (`R2`/`R3` on `4|a|a` and `4|a|a|1`, `R13` on `4|a|a|1` and `4|a|a|3`/`6`/`8`/`9`/`10`, `R22` `$Entrepreneur_i^{HighCap}$` on `4|a|a|2|i`, keeping separate `**变异来源**` and `**比较**` headings for `EM4`).
     - Anchored all 7 loose limitation/external-validity cards (`4|a|a|3|a`, `4|a|a|6|a`, `4|a|a|8|b`, `4|a|a|9|a`, `4|a|a|9|b`, `4|a|a|10|b`, `4|a|b`; `loose` 7 → 0).
     - Rebuilt 2-level `**结构**` and added `**分部**` (4 parts covering 8 chapters) on head `4|a`.
2. **`gottlieb2022career/7|c` (empirical, 31 parts, 0 moves, 3 cards edited):**
   - Added `**直觉**` leads to `7|c|a|1` (`设计`) and `7|c|a|2` (`规格：基准`); anchored `7|c|b` (`见设计 [[gottlieb2022career/7|c|a|1]]。`, `loose` 1 → 0).
3. **`gottlieb2022career/8|b` (empirical, 39 parts, 0 moves, 5 cards edited):**
   - Added `**直觉**` leads to `8|b|a` (`数据`), `8|b|a|1` (`假说`), `8|b|a|2` (`设计`), `8|b|a|3` (`规格：基准`); anchored `8|b|b` (`见设计 [[gottlieb2022career/8|b|a|2]]。`, `loose` 1 → 0).
4. **`gottlieb2022career/8|d` (empirical, 31 parts, 0 moves, 5 cards edited):**
   - Added `**直觉**` leads to `8|d|a` (`数据`), `8|d|a|1` (`假说`), `8|d|a|2` (`设计`), `8|d|a|3` (`规格：基准`); anchored `8|d|b` (`见设计 [[gottlieb2022career/8|d|a|2]]。`, `loose` 1 → 0).

### Stage 3 Gate verification (post term-relink)

- **Term relink (`zt-digest term-link -Step relink -CitationKey gottlieb2022career`):** `ok: true, complete: true, cards_changed: 9, links_added: 19, links_repaired: 0, cards_refused: 0`; final `-PlanOnly` confirmed `cards_changed: 0, links_added: 0`.
- **`list_broken_links` (`agent`):** `[]` (0 broken links total, 0 in `gottlieb2022career/`). Tree card count: **471 → 463** (−8 dissolved inner chapter wrappers).

| Model | Members (main/side/off) | Loose | `--stage restructure` | `--network` | `--settings` | `--skip-shared` | `--premises` |
| --- | --- | ---: | --- | --- | --- | --- | --- |
| `gottlieb2022career/4\|a` | 138 (11/126/1) | 0 | pass (`rc=0`, `self_sufficient true`, `evidence_bound true`) | `network: clean` (12 blocks, 4 layers, 4 parts, `rc=0`) | clean (`rc=0`) | `faults: 0` (`rc=0`, advisory R20 only) | 10 pointers/SE hits (0 hidden uses) |
| `gottlieb2022career/7\|c` | 31 (6/24/1) | 0 | pass (`rc=0`, `self_sufficient true`, `evidence_bound true`) | `network: clean` (7 blocks, 6 layers, `rc=0`) | clean (`rc=0`) | `faults: 0` (`rc=0`, advisory R20 only) | none (`rc=0`) |
| `gottlieb2022career/8\|b` | 39 (7/31/1) | 0 | pass (`rc=0`, `self_sufficient true`, `evidence_bound true`) | `network: clean` (8 blocks, 7 layers, `rc=0`) | clean (`rc=0`) | `faults: 0` (`rc=0`, advisory R20 only) | none (`rc=0`) |
| `gottlieb2022career/8\|d` | 31 (7/23/1) | 0 | pass (`rc=0`, `self_sufficient true`, `evidence_bound true`) | `network: clean` (8 blocks, 6 layers, `rc=0`) | clean (`rc=0`) | `faults: 0` (`rc=0`, advisory R20 only) | none (`rc=0`) |

- **`4|a` `--premises` audit (10 hits, 0 hidden forward uses):** `4|a|a|a` (`变量表`, 2 rows naming `(1)` and `(4)`), `4|a|a|1|c` & `4|a|a|1|d` (`怎样进入模型` pointers to Eqs. `(2)`–`(3)`), `4|a|a|1|o` (Table 9 Col. `(1)` + pointer to Eq. `(3)`), `4|a|a|2` & `4|a|a|2|a` (nested-equation pointers `(1) → (2) → (3)` across Side variants), and `4|a|a|5|a` (Table 3 Panel A SE `(32.30)` matching `4|a|a|5|d` title).
- **Handoff:** Ready for Stage 4 (`zt-model-import` + `zt-evidence-dialogue`) on all surviving models.

## Stage 4: imports and explanations

- **Date:** 2026-11-27
- **Archive:** Live box `D:\zt` (all writes serialized via `zt_mutex.py`)
- **Pre-Stage-4 backup:** `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3`
- **Scope:** `zt-model-import` (steps 1–9) + `model-explain` on all 6 surviving models in lender order: concept lenders `5|a`, `8|a`, empirical lenders `4|a`, `7|c`, then empirical borrowers `8|b`, `8|d`. All display formulas were verified byte-for-byte unchanged before every write.

### Per-model changes

1. **`gottlieb2022career/5|a` (concept lender, 5 → 3 parts):**
   - Fixed 2 `false_share` links in `**关联**` (`4|a|a|6`, `4|a|a|8` → `[[gottlieb2022career/4|a]]` naming Ch. 4 and Ch. 6 in prose). False shares `2 → 0`, tolerated `3 → 0`, members `5 → 3` (all own). Lends `5|a|a|2` (`结论`) `单卡` to `4|a`.
2. **`gottlieb2022career/8|a` (concept lender, 5 → 4 parts):**
   - Fixed 1 `false_share` link in `**关联**` (`4|a|a|10` → `[[gottlieb2022career/4|a]]` naming Ch. 8 in prose). False shares `1 → 0`, tolerated `2 → 0`, members `5 → 4` (all own). Lends `8|a|a|2` (`结论`) `单卡` to `4|a`, `8|b`, and `8|d`.
3. **`gottlieb2022career/4|a` (empirical, 138 → 140 parts, +2 shared):**
   - Wired `4|a|a|8|a` (`假说：假期增幅越大效应越强`) to rest on `5|a|a|2` (`依赖 [[gottlieb2022career/5|a|a|2]]`，`（经 **引入**）`) and `4|a|a|10|a` (`假说：直接机制行业比较静态`) to rest on `8|a|a|2` (`依赖 [[gottlieb2022career/8|a|a|2]]`，`（经 **引入**）`).
   - `imports.mjs apply` wrote `单卡 [[gottlieb2022career/5|a|a|2]]` and `单卡 [[gottlieb2022career/8|a|a|2]]` (`tolerated 0 → 2`, advice on `5|a|a|1` and `8|a|a|1`); both intentionally kept `单卡` per empirical-hypothesis rule (a test takes the prediction, not its proof). Refined `**引入**` descriptions. Lends `4|a|a` to `8|b` (`单卡`) and `8|d` (`整块`).
4. **`gottlieb2022career/7|c` (empirical lender, 31 parts, 0 imports):**
   - Independent LAD longitudinal tax panel (`7|c|a`) with loose net self-employment measure (`7|c|a|b`); no outside premises, no imports. Lends `7|c|a` to `8|b` (`单卡`) and `8|d` (`整块`).
5. **`gottlieb2022career/8|b` (empirical, 39 → 42 parts, +3 shared):**
   - Wired `8|b|a|1` (`假说`) to rest on `8|a|a|2` (`依赖`，`（经 **引入**）`) and `8|b|a` (`数据` `**起点**`) to rest on `7|c|a` (LAD panel, subsetted to wage-stayers) and `4|a|a` (2006 Census 30-day sample).
   - `imports.mjs apply` wrote `单卡 [[gottlieb2022career/7|c|a]]`, `单卡 [[gottlieb2022career/4|a|a]]`, and `单卡 [[gottlieb2022career/8|a|a|2]]` (`tolerated 0 → 1`, advice on `8|a|a|1`; `8|a|a|2` kept `单卡` per empirical-hypothesis rule; `7|c|a` and `4|a|a` have 0 tolerated premises and stay `单卡` since `8|b` does not use the entrepreneurship outcome measures). Refined `**引入**` descriptions.
6. **`gottlieb2022career/8|d` (empirical, 31 → 41 parts, +10 shared):**
   - Wired `8|d|a|1` (`假说`) to rest on `8|a|a|2` (`依赖`，`（经 **引入**）`) and `8|d|a` (`数据` `**起点**`) to rest on `7|c|a` + `7|c|a|b` (LAD 30-day sample + loose net self-employment measure) and `4|a|a` + `4|a|a|c` (2006 Census 30-day sample + income-based entrepreneurship measure).
   - `imports.mjs apply` wrote `整块 [[gottlieb2022career/7|c|a]]` (absorbing `7|c|a|b`, +3 shared), `整块 [[gottlieb2022career/4|a|a]]` (absorbing `4|a|a|c`, +6 shared), and `单卡 [[gottlieb2022career/8|a|a|2]]` (+1 shared; `tolerated 0 → 1`, advice on `8|a|a|1`; kept `单卡` per empirical-hypothesis rule). Refined `**引入**` descriptions.

### Step 8 (`model-explain`) and Term relink

- **`zt-digest model-explain -Step plan -Force`** on all 4 empirical models (`4|a`, `7|c`, `8|b`, `8|d`): `nothing to explain` (`parts: 0, jobs: []`; 0 DeepSeek jobs).
- **`zt-digest term-link -Step relink -CitationKey gottlieb2022career`** (under `zt_mutex.py`): `ok: true, complete: true, cards_changed: 4, links_added: 5, links_repaired: 0, cards_refused: 0`; final `-PlanOnly` confirmed `cards_changed: 0, links_added: 0`.
- **`list_broken_links` (`agent`):** `[]` (0 broken links in `gottlieb2022career/`).

### Stage 4 Final Gate verification (post term-relink)

| Model | Level | Imports (`**引入**`) | `imports check` | `--stage final` | `--settings` | `--network` | `--skip-shared` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `gottlieb2022career/5\|a` | concept | none | `rc=0`, 0 false shares, 0 tolerated | pass (`members 3`, `loose 0`, `self_sufficient true`) | n/a | n/a | n/a |
| `gottlieb2022career/8\|a` | concept | none | `rc=0`, 0 false shares, 0 tolerated | pass (`members 4`, `loose 1`, `self_sufficient true`) | n/a | n/a | n/a |
| `gottlieb2022career/4\|a` | empirical | `单卡 5\|a\|a\|2`, `单卡 8\|a\|a\|2` | `rc=0`, 0 false shares, 2 tolerated | pass (`members 140`, `shared:2`, `loose 0`, `self_sufficient true`, `evidence_bound true`) | clean (`rc=0`) | clean (14 blocks, 4 layers, `rc=0`) | `faults: 0` (`rc=0`, R20 adv only) |
| `gottlieb2022career/7\|c` | empirical | none | `rc=0`, 0 false shares, 0 tolerated | pass (`members 31`, `loose 0`, `self_sufficient true`, `evidence_bound true`) | clean (`rc=0`) | clean (7 blocks, 6 layers, `rc=0`) | `faults: 0` (`rc=0`, R20 adv only) |
| `gottlieb2022career/8\|b` | empirical | `单卡 7\|c\|a`, `单卡 4\|a\|a`, `单卡 8\|a\|a\|2` | `rc=0`, 0 false shares, 1 tolerated | pass (`members 42`, `shared:3`, `loose 0`, `self_sufficient true`, `evidence_bound true`) | clean (`rc=0`) | clean (11 blocks, 8 layers, `rc=0`) | `faults: 0` (`rc=0`, R20 adv only) |
| `gottlieb2022career/8\|d` | empirical | `整块 7\|c\|a`, `整块 4\|a\|a`, `单卡 8\|a\|a\|2` | `rc=0`, 0 false shares, 1 tolerated | pass (`members 41`, `shared:10`, `loose 0`, `self_sufficient true`, `evidence_bound true`) | clean (`rc=0`) | clean (11 blocks, 7 layers, `rc=0`) | `faults: 0` (`rc=0`, R20 adv only) |

- **Handoff:** Ready for `zt-evidence-dialogue` on `4|a`, `7|c`, `8|b`, `8|d` and `zt-literature-organize` on `gottlieb2022career`.

## Stage 5: evidence dialogue

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; judged by Gemini 4 Neon Medium swarm workers, 0 DeepSeek cost). Backup: `D:\zt\backups\zt-before-iof10-stage5-dialogue-2026-11-27.sqlite3`.

| Model | Unique pairs judged | Same estimand (印证 / 反驳 / 对照) | Cards written | Check |
|---|---|---|---|---|
| `4|a` | 509 | 8 (0 / 0 / 8) | 6 | `{"head":"gottlieb2022career/4|a","broken":0,"proposed_again":0,"ok":true}` |
| `7|c` | 8 | 0 (0 / 0 / 0) | 0 | `{"head":"gottlieb2022career/7|c","broken":0,"proposed_again":0,"ok":true}` |
| `8|b` | 231 | 0 (0 / 0 / 0) | 0 | `{"head":"gottlieb2022career/8|b","broken":0,"proposed_again":0,"ok":true}` |
| `8|d` | 84 | 0 (0 / 0 / 0) | 0 | `{"head":"gottlieb2022career/8|d","broken":0,"proposed_again":0,"ok":true}` |

- Term relink: `{"ok":true,"complete":true,"book":"gottlieb2022career","cards_changed":4,"links_added":4,"links_repaired":0,"cards_refused":0,"rounds":1,"applied":true}`; `-PlanOnly`: `{"ok":true,"complete":true,"book":"gottlieb2022career","cards_changed":0,"links_added":0,"links_repaired":0,"cards_refused":0,"rounds":0,"applied":false}`.

## Stage 6: literature organize

- **Date:** 2026-11-27
- **Archive:** Live box `D:\zt` (all writes serialized via `zt_mutex.py`, 0 DeepSeek calls)
- **Pre-Stage-6 backup:** `D:\zt\backups\zt-before-iof10-stage6-organize-2026-11-27.sqlite3`

### Source divisions

From the digested source headings (引言 through §7 before References): 引言; 1. 数据; 2. 加拿大的产假政策; 3. 实证策略; 4. 因果解释与反事实分析 (4.1 降低职业风险的因果效应, 4.2 反事实路径); 5. 实证结果 (5.1 实证设计的有效性检验, 5.2 主要实证发现, 5.3 稳健性检验, 5.4 省际异质性分析, 5.5 创业企业质量, 5.6 职位保留与收入替代的区分, 5.7 短期效应分析); 6. 作用机制检验 (6 开篇, 6.1 排除间接机制, 6.2 排除间接机制, 6.3 验证直接机制); 7. 结论; plus 术语表 and 参考文献 at the tail.

### Final tree (top line and divisions)

```
1  引言 (old 1, 14)                          c §引言 chain · d structure Card
2  1. 数据 (old 2, 9)                        b–c findings · d §1 chain
3  2. 加拿大的产假政策 (old 3, 7)            a §2 chain + 2 events
4  3. 实证策略 (old 4, 146)                  a empirical model 4|a (139 tree cards, 140 members) · b–c findings · d §3 chain (detached from 4|a)
5  4. 因果解释与反事实分析 (old 5, rewrite, 8) a 4.1 (old 5|a|1..2) · b 4.2 (old 5|a|3 + concept model 5|b|a)
6  5. 实证结果 (old 6, rewrite, 67)          a 5.1 · b 5.2 · c 5.3 · d 5.4 · e 5.5 · f 5.6 · g 5.7 (with empirical model 6|g|b)
7  6. 作用机制检验 (old 8, rewrite, 93)      a 6 开篇 · b concept model 7|b · c 6.1 (with empirical model 7|c|a) · d 6.2 · e 6.3 (with empirical model 7|e|b)
8  7. 结论 (from old 8|e|6, 1)
9  术语表 (from 1|b, 27)
10 参考文献 (from 1|a, 89)
```

### Crossers, wrapper dissolution, and moves

- Old `1` held §引言 and both lists: Term list (`1|b`) and Cited list (`1|a`) moved to the tail (`9`, `10`); structure Card `1|d` stays under `1`.
- Reading-note chains detached from Model Cards: `4|a|1..4|a|4` (§3) moved to `4|d..4|d|3`; `5|a|1..5|a|2` (§4.1) and `5|a|3` (§4.2) moved off concept model `5|a`, with `5|a` placed under `5|b` as `5|b|a`.
- Digest-group split of §5 (old `6` §5.1–§5.3 and old `7` §5.4–§5.7) merged into one top Card `6` (`5. 实证结果`) with `6|a`–`6|g` (5.1–5.7) in source order; redundant digest-group wrapper old `7` deleted `into` `6` (tree count `463 → 462`).
- Crosser old `8` (§6 + §7) split: `8|e|6` promoted to top Card `8` (`7. 结论`); §6 organized under `7` with `7|a` (6 开篇), `7|b` (concept model), `7|c` (6.1 + model `7|c|a` + 3 findings), `7|d` (6.2), and `7|e` (6.3 + model `7|e|b` + 4 findings).
- Summary leads (`leads.mjs --commit`): rewrote `5` (§4), `6` (§5), and `7` (§6) with `小节索引：`, `在全书中的位置：`, and `来源：[[gottlieb2022career]] §…`. Updated section/subdivision title prefixes (`4`, `8`, `5|a`–`5|b`, `6|a`–`6|g`, `7|a`, `7|c`–`7|e`), root index `gottlieb2022career`, and structure Card `gottlieb2022career/1|d` (listing the 8 body divisions and 6 surviving models at final addresses).

### Final Model Card addresses (6 models)

| Model Card | Level | Members | `--stage final` | `--settings` | `--network` |
| --- | --- | --- | --- | --- | --- |
| `gottlieb2022career/4\|a` | empirical | 140 (138 own + 2 shared) | pass (`rc=0`) | clean (`rc=0`) | clean (`rc=0`) |
| `gottlieb2022career/5\|b\|a` | concept | 3 (own) | pass (`rc=0`) | n/a | n/a |
| `gottlieb2022career/6\|g\|b` | empirical | 31 (own) | pass (`rc=0`) | clean (`rc=0`) | clean (`rc=0`) |
| `gottlieb2022career/7\|b` | concept | 4 (own) | pass (`rc=0`) | n/a | n/a |
| `gottlieb2022career/7\|c\|a` | empirical | 42 (39 own + 3 shared) | pass (`rc=0`) | clean (`rc=0`) | clean (`rc=0`) |
| `gottlieb2022career/7\|e\|b` | empirical | 41 (31 own + 10 shared) | pass (`rc=0`) | clean (`rc=0`) | clean (`rc=0`) |

### Stage 6 Gate verification (post term-relink)

- **Term relink (`zt-digest term-link -Step relink -CitationKey gottlieb2022career`):** `ok: true, complete: true`, final `-PlanOnly`: `cards_changed: 0, links_added: 0, links_repaired: 0, cards_refused: 0`.
- **`zt-check --tree gottlieb2022career`:** `no fault` (`rc=0`); `--snapshot` diff shows only the expected −1 deleted digest-group wrapper (`463 → 462`) and the two Direct-chain reading-note detachments (`4|a` `143 → 139`, `5|b|a` `7 → 4`; 0 model parts moved).
- **`zt-check --cards` (20 edited root, structure, lead, and section/subdivision head Cards):** `faults: 0` (`rc=0`, advisory L1/L2/I4 only).
- **`model-survey.mjs gottlieb2022career`:** 6 surviving Model Cards (`4|a`, `5|b|a`, `6|g|b`, `7|b`, `7|c|a`, `7|e|b`), `rc=0`.
- **`list_broken_links` (`agent`):** `{"total_broken":0,"gottlieb_broken":[]}`.


## Stage 7: illustrate

Not applicable (`gottlieb2022career` holds 4 empirical models and 2 concept models, 0 formal models; Stage 7 applies to formal models only).
