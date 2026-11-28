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
