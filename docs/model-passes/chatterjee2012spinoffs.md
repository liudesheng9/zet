# Model passes: chatterjee2012spinoffs

Owner: Xingtong Chen

Chatterjee and Rossi-Hansberg (2012), *Spinoffs and the Market for Ideas* (`International Economic Review`, 53(1), 53–93). Digest source: `D:\zt-digest\chatterjee2012spinoffs\book.zh.md`.

## Stage 1: fold

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-postdigest-2026-11-27.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok).

### Decision by nature (7 initial Model Cards -> 2 surviving models)

The digest split the paper's single formal dynamic screening and market-for-ideas theory across 6 formal Model Cards (`2|a`, `3|a`, `5|a`, `7|a`, `7|b`, `7|c`) by section/appendix group, plus 1 empirical calibration and entry-size test Model Card (`5|b`).

| Model Card | Model (surviving head) | Level | Initial Parts | Decision & Reason |
|---|---|---|---|---|
| `2\|a` | formal (`2\|a`) | formal | 23 | **Surviving head.** Baseline dynamic screening & market-for-ideas model (§2.1–§2.2): infinite-horizon discrete time, workers ($w$) and entrepreneurs managing $N$ projects with profit $\pi(S,N)=N(S-w)$, private idea mean $\mu \sim H(\mu)$, realized $P \sim F_\mu(P)$, entrepreneur-specific projects, uniform price $Z$, Bellman equations $W(S,N)$, $V(\mu,S,N)$, $W_0$, $V_0(\mu)$, Lemmas 1–4. |
| `3\|a` | formal (`2\|a`) | formal | 19 | **Fold into `2\|a`.** Continues `2\|a`'s exact primitives and Bellman equations in §2.3–§3, adding market clearing eq. (3) and specializing utility to risk-neutral linear $u(c_t)=c_t$ to obtain closed-form Propositions 1–2. |
| `5\|a` | formal (`2\|a`) | formal | 31 | **Fold into `2\|a`.** Continues the linear-utility equilibrium of `3\|a` (`2\|a`) in §5.1 with linear idea-learning function $\tilde{\gamma}(\lambda, N)=\lambda N$ and market clearing eq. (8) to derive the invariant Yule firm-size distribution $\delta_N$, Gibrat's Law, and Pareto upper tail (Corollary 1, Propositions 4–5). |
| `5\|b` | empirical (`5\|b`) | empirical | 30 | **Own model (stays separate).** §5.2 empirical calibration and out-of-sample entry-size test on 2000 US SUSB and 1989–2005 SBA data. Per `zt-model-fold` rules, a theory and its empirical calibration/test are never folded together; `5\|b` will import `2\|a` via `**引入**` in Stage 4. |
| `7\|a` | formal (`2\|a`) | formal | 24 | **Fold into `2\|a`.** Appendix A.1–A.6 complete proofs of `2\|a` (Lemmas 1–4 via Blackwell contraction mapping on $\mathcal{C}$) and `3\|a` (Propositions 1–2 via undetermined coefficients). Same model, proofs of its theorems. |
| `7\|b` | formal (`2\|a`) | formal | 13 | **Fold into `2\|a`.** Appendix A.7 CARA exponential utility variant $u(c_t)=-ae^{-bc_t}$ of the exact same dynamic screening framework (introduced in §2 as the second analytical specialization alongside linear utility). |
| `7\|c` | formal (`2\|a`) | formal | 18 | **Fold into `2\|a`.** Appendix B extension of the same framework where entrepreneurs also generate ideas with probability $\lambda$ and spinoff requires exogenous resource cost $f>0$ replacing endogenous option value $f_0$ (introduced in §1 fn. 2 & §2 fn. 8; proves Proposition 7 and asymptotic Pareto upper tail). |

### Fold actions

All 5 formal heads (`3|a`, `5|a`, `7|a`, `7|b`, `7|c`) were folded into `chatterjee2012spinoffs/2|a` one card at a time in book order, with all writes serialized under `zt_mutex.py` and no mathematics or links deleted:

1. **`3|a` -> `2|a|a|3`** (`facet: "第 3 章：线性效用下的长期均衡刻画"`, `emphasis: "core"`, `level: ""`):
   - Stripped `**结构**` (no `**引入**` lines present), masked 9 card links in `3|a` and incoming links to `3|a` in remaining heads (`5|a`, `7|a`, `7|b`, `7|c`).
   - Moved `3|a`'s 9 Idea-note descendants (`3|a|1..6` and footnotes `3|a|2|a`, `3|a|4|a`, `3|a|5|a`) back onto section anchor `chatterjee2012spinoffs/3` (`3|b`..`3|b|5`).
   - Parked `3|a|a`, `3|a|b` (19 parts) under `2|a`, retyped `3|a`, moved `3|a` alone to `2|a|a|3`, moved its 19 parts back under `2|a|a|3|a`..`b`, and restored masked links via rename map.
2. **`5|a` -> `2|a|a|4`** (`facet: "第 4 章：企业规模不变分布、尤尔分布与吉布拉法则"`, `emphasis: "core"`, `level: ""`):
   - Stripped `**结构**`, masked 10 card links in `5|a` and incoming links to `5|a` in remaining heads (`5|b`, `7|c`).
   - Parked `5|a|a`, `5|a|b` (31 parts) under `2|a`, retyped `5|a`, moved `5|a` alone to `2|a|a|4`, moved its 31 parts back under `2|a|a|4|a`..`b`, and restored masked links via rename map.
3. **`7|a` -> `2|a|a|5`** (`facet: "附录章：基准模型与线性均衡证明（引理 1–4、命题 1–2）"`, `emphasis: "core"`, `level: ""`):
   - Stripped `**结构**`, masked 9 card links in `7|a`, parked 24 parts under `2|a`, retyped `7|a`, moved alone to `2|a|a|5`, restored 24 parts under `2|a|a|5|a`..`b`, and restored masked links via rename map.
4. **`7|b` -> `2|a|a|6`** (`facet: "附录章：CARA 指数效用变体"`, `emphasis: "core"`, `level: ""`):
   - Stripped `**结构**`, masked 5 card links in `7|b`, parked 13 parts under `2|a`, retyped `7|b`, moved alone to `2|a|a|6`, restored 13 parts under `2|a|a|6|a`..`b`, and restored masked links via rename map.
5. **`7|c` -> `2|a|a|7`** (`facet: "附录章：企业家产创意与外生进入成本扩展"`, `emphasis: "core"`, `level: ""`):
   - Stripped `**结构**`, masked 5 card links in `7|c`, parked 18 parts under `2|a`, retyped `7|c`, moved alone to `2|a|a|7`, restored 18 parts under `2|a|a|7|a`..`b`, and restored masked links via rename map.
6. **Empirical head `5|b` link update**:
   - Restored the masked `**关联**` link in `chatterjee2012spinoffs/5|b` (`参照 [[chatterjee2012spinoffs/5|a]]` -> `参照 [[chatterjee2012spinoffs/2|a]]`) so `5|b` references the surviving formal model head cleanly without creating a legacy shared part.

### Surviving heads

| Address | Level | Parts | Summary |
|---|---|---|---|
| `chatterjee2012spinoffs/2\|a` | formal | 133 | Complete formal dynamic screening & market-for-ideas model: general utility (§2), linear utility long-run equilibrium (§2.3–§3), invariant Yule firm-size distribution & Gibrat's Law (§5.1), proofs (App. A.1–A.6), CARA variant (App. A.7), and entrepreneur-generated ideas + exogenous entry cost extension (App. B). |
| `chatterjee2012spinoffs/5\|b` | empirical | 30 | US SUSB (2000) & SBA (1989–2005) quantitative calibration of the rescaled Yule distribution, 2-digit NAICS team-size estimation, and out-of-sample entry-size test (§5.2). |

### Gate results

- `node E:\zet\skills\zt-model-fold\scripts\model-survey.mjs chatterjee2012spinoffs`: **PASS** — lists exactly 2 Model Cards (`2|a` formal, 133 parts; `5|b` empirical, 30 parts), 2 candidate groups (`G1: 2|a`, `G2: 5|b`), 0 structural joins.
- `zt-check --model 'chatterjee2012spinoffs/2|a' --stage fold`: **PASS** (exit 0) — `members 133 {main:8, off:1, side:124}`, 0 Stage 1 blocking faults (only expected deferred Stage 2 `setting_dependency` ×13 and Stage 3 `unanchored` ×5 on the folded chapters `2|a|a|3..7`).
- `zt-check --model 'chatterjee2012spinoffs/5|b' --stage fold`: **PASS** (exit 0) — `members 30 {main:7, off:1, side:22}`, `self_sufficient true`, `evidence_bound true`.
- `list_broken_links`: **PASS** — 0 broken links (total archive and `chatterjee2012spinoffs/`); tree card count 293 unchanged; 0 leftover link masks.
- `zt-digest term-link -Step relink -CitationKey chatterjee2012spinoffs`: **PASS** — 1 card changed, 1 link added; follow-up `-PlanOnly` confirmed 0 changes (`cards_changed: 0, links_added: 0, links_repaired: 0`).

### Handoff to next stages

- Stage 2 (`zt-model-settings`): `chatterjee2012spinoffs/2|a` (consolidate settings/assumptions from `2|a|a|3..7` into `2|a|a` and disambiguate duplicate `假设 1`/`假设 2` labels), `chatterjee2012spinoffs/5|b`.
- Stage 3 (`zt-model-restructure`): `chatterjee2012spinoffs/2|a`, `chatterjee2012spinoffs/5|b`.
- Stage 4 (`zt-model-import` + `zt-evidence-dialogue`): `chatterjee2012spinoffs/2|a`, `chatterjee2012spinoffs/5|b` (add `**引入**` from `5|b` to `2|a`).

## Stage 2: settings

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; all writes serialized under `zt_mutex.py`). Pre-Stage-2 backup: `D:\zt\backups\zt-before-iof10-stage2-settings-2026-11-27.sqlite3`.

### 2\|a (formal, 133 -> 128 parts)

All 5 folded setting subtrees from chapters `2|a|a|3..7` were gathered into `2|a|a`'s Side dependency tree in one atomic `apply_batch` (20 items: 95 cards moved, 5 redundant notation cards deleted `into` `2|a|a|a`, 417 link macros rewritten, `links_that_will_break: []`), while promoting each folded chapter's Direct sub-chapter chain into slot `a` ahead of its `局限` card (`b`):

```
2|a|a    设定：偏好、职业与利润结构                                             原始设定
├ a      符号表与读法（统一符号表 47 行 + 统一用语 17 条，合并 3|a|a–7|a|a）
├ b      假设 1：不可复制技术与获悉分解                                          ← 2|a|a
├ c      假设 2：私人信息与双层随机分布                                          ← 2|a|a
├ d      假设 3：企业家专属性与契约排除                                          ← 2|a|a
├ e      解概念：两方动态规划最优条件                                            ← 2|a|a, b, c, d
├ f      设定：变体一：线性效用与长期均衡框架（原 2|a|a|3|a）                    ← 2|a|a, e
│ ├ f|a  假设 L1：线性获悉概率形式（原 3|a|b）                                   ← f, b
│ ├ f|b  假设 L2：无界效用最优性与正期权支撑（原 3|a|c）                         ← f, c
│ └ f|c  均衡定义：长期均衡与市场出清（原 3|a|d）                                ← f, b, e
├ g      设定：变体二：规模演进与宏观就业框架（原 2|a|a|4|a）                    ← 2|a|a, f, f|c
│ ├ g|a  假设 S1：线性获悉与规模上界（原 4|a|b）                                 ← g, f|a
│ ├ g|b  假设 S2：总就业标准化与确定性宏观演进（原 4|a|c）                       ← g
│ └ g|c  均衡定义：线性获悉出清与流平衡稳态（原 4|a|d）                          ← g, g|a, g|b, f|c
├ h      设定：函数空间 \mathcal{C} 与贝尔曼算子 T（原 2|a|a|5|a）               ← 2|a|a, b, c, e
│ ├ h|a  假设 A1：有界凹性与线性证明正则条件（原 5|a|b）                         ← h, c, f|b
│ └ h|b  解概念：不动点、无差异阈值与竞争零剩余（原 5|a|c）                      ← h, h|a, f|c
├ i      设定：变体三：CARA 指数效用偏好特化（原 2|a|a|6|a）                     ← 2|a|a, e
│ ├ i|a  假设 C1：正定系数、价格分母为正与无界最优性（原 6|a|b）                 ← i, c
│ └ i|b  解概念：CARA 贝尔曼不动点与竞争出清条件（原 6|a|c）                     ← i, i|a, e
└ j      设定：变体四：全员产创意与外生进入成本 f>0（原 2|a|a|7|a）              ← 2|a|a–e
  ├ j|a  假设 B1：外生正成本 f>0 与线性效用获悉设定（原 7|a|b）                  ← j, f, f|a
  └ j|b  解概念：双通道贝尔曼方程与修正市场出清（原 7|a|c）                      ← j, j|a, b, e, f|c
```

- **Merged / deleted (`into` `2|a|a|a`)**: the 5 folded notation cards (`2|a|a|3|a|a`, `4|a|a`, `5|a|a`, `6|a|a`, `7|a|a`) were merged into `2|a|a|a` (47 symbol rows covering every setting symbol and derivation auxiliary symbol including $\nu$, $\delta_N$, $\overline{N}$, $\mathcal{C}$, $T$, $f_0$, $f$, $a$, $b$, $e$, $V^E$, $\mu_L^E$, $dF_\mu(P)$, $dH(\mu)$, $d$, plus 17 deduplicated `**统一用语**` entries) and deleted `into` `2|a|a|a`.
- **Disambiguation & format**: disambiguated all 7 duplicate `假设 1`/`假设 2` labels into `假设 L1`, `假设 L2`, `假设 S1`, `假设 S2`, `假设 A1`, `假设 C1`, `假设 B1`; standardized `**起点**`/`**设定**` (or `**假设**`/`**均衡定义**`/`**解概念**` + `**作用域**`)/`**直觉**`/`**说明**` across all moved setting cards; removed restated base statements on `i|b`, `j`, `j|b` (`repeat` faults) and forward chapter link on `i|a` (`order` fault).
- **Chapter faults fixed**: reworded proof-by-contradiction step in `2|a|a|1|a|a` (`反证：若…`) and linked `假设 3` -> `[[2|a|a|d]]` (`stated` + `cite`); linked `假设 2` -> `[[2|a|a|g|b]]` in `2|a|a|4|a|b|a|1` (`cite`); linked `假设 1` -> `[[2|a|a|h|a]]` in `2|a|a|5|a` (`cite`).
- **Formula preservation & `**结构**`**: verified programmatically byte-for-byte that every `$$...$$` display formula and `\tag{...}` survived unchanged across all 24 edited cards; rebuilt `2|a`'s `**结构**` section (128 parts).

### 5\|b (empirical, 30 parts)

- Fixed both `start` faults on the data tree (`5|b|a|b` and `5|b|a|c`) by adding a one-line summary, `**起点**` (`依赖 [[chatterjee2012spinoffs/5|b|a]]`), and `**内容**` while preserving all existing text and links. No moves or structural changes needed.

### Term relink & Gate results

- `zt-digest term-link -Step relink -CitationKey chatterjee2012spinoffs`: `cards_changed: 7, links_added: 21, links_repaired: 0, cards_refused: 0`; follow-up `-PlanOnly`: `cards_changed: 0, links_added: 0, links_repaired: 0`.
- `zt-check --model 'chatterjee2012spinoffs/2|a' --settings --symbols`: **PASS** (exit 0) — `settings: all in the setting part` (0 faults; only advisory `balance` and `uses`).
- `zt-check --model 'chatterjee2012spinoffs/2|a' --stage settings`: **PASS** (exit 0) — `members 128 {"main":8,"off":1,"side":119}`, `formal_structured true; stage(settings) pass` (only expected Stage 3 deferred `unanchored` on folded chapters `2|a|a|3..7`).
- `zt-check --model 'chatterjee2012spinoffs/5|b' --settings`: **PASS** (exit 0) — `settings: data tree and specification ladder are clean`.
- `zt-check --model 'chatterjee2012spinoffs/5|b' --stage settings`: **PASS** (exit 0) — `members 30 {"main":7,"off":1,"side":22}`, `self_sufficient true; evidence_bound true; stage(settings) pass`.
- `list_broken_links` (`agent`): **PASS** — `0` broken links total, `0` for `chatterjee2012spinoffs/`.


## Stage 3: restructure

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; all writes serialized under `zt_mutex.py`). Pre-Stage-3 backup: `D:\zt\backups\zt-before-iof10-stage3-restructure-2026-11-27.sqlite3`.

### 2\|a (formal, 128 -> 117 parts): the intuition spine

Dissolved all 11 redundant nested sub-chapter wrapper cards left by the Stage 1 fold (`3|a`, `3|a|1`; `4|a`, `4|a|1`, `4|a|2`; `5|a`, `5|a|1`, `5|a|2`; `6|a`; `7|a`, `7|a|1`) in one atomic `apply_batch` (`26` result/calibration/experiment subtrees promoted to direct Side branches of `2|a|a|3..7`, `11` empty wrappers deleted `into` their parent chapters, `5` chapter Side lists reordered so each chapter's `局限` card sits last; `links_that_will_break: []`).

```
2|a|a      设定：偏好、职业与利润结构 (a 符号表与读法, b–e 基准假设与解概念, f 线性变体, g 规模演进变体, h 证明算子, i CARA 变体, j 全员产创意扩展; 23 cards)
2|a|a|1    第 1 章：激励相容定价与在位筛选 (§2.1; a 统一定价, b–d 引理 1–3, e 在位筛选分段方程)                         ← e
2|a|a|2    第 2 章：工人衍生与事后留存门槛 (§2.2; a 引理 4, b 工人衍生分段价值, c 事后留存退出门槛)                      ← 1, e
2|a|a|3    第 3 章：线性效用下的长期均衡刻画 (§2.3–§3; a 出清不确定性, b 命题 1, c 创意市场必要性, d 方程 (5)–(7), e 命题 2, f 局限)  ← 2, f, f|c
2|a|a|4    第 4 章：企业规模不变分布、尤尔分布与吉布拉法则 (§5.1; a 出清比率 (9), b 转移核与总量扩张, c 命题 3, d 推论 1, e 命题 4, f 均值回归, g 命题 5, h 校准图 7, i 数值实验图 7, j 局限) ← 3, g, g|c
2|a|a|5    第 5 章：基准与线性均衡证明（附录 A.1–A.6; a–d 引理 1–4 证明, e–f 命题 1 上下半证明, g 命题 2 证明, h 局限) ← 3, 4, h|b
2|a|a|6    第 6 章：CARA 指数效用变体（附录 A.7; a 命题 6 含两条推导链与边界, b 局限)                                  ← 2, i, i|b
2|a|a|7    第 7 章：企业家产创意与外生进入成本扩展（附录 B; a 均衡比率 θ=1, b 命题 7, c 去专属性, d 渐近帕累托收敛, e 局限) ← 4, j, j|b
2|a|b      局限：无储蓄、无融资摩擦与固定项目规模 (off main line, anchored to 2|a|a)
```

- **Chapter card rewrites (`2|a|a|3..7`)**: converted all 5 folded former-Model-Card chapters to clean Chapter Card format (`第 3–7 章` titles/facets, `依赖` anchors, `**直觉**` >= 20 chars, `**本章结果**` TOC, `**承上启下**`, `来源`), preserving all three cross-book links (`akcigit2025transformative/17|a`, `aghion2026nobel/2|b|1` on `2|a|a|4`; `atkinson2015lectures/7|a|c|b|3` on `2|a|a|6`) and keeping App. A (`第 5 章`) as a dedicated anchored formal-proofs chapter after `第 4 章` because App. A.6 (`2|a|a|5|g`) depends on Corollary 1 (`2|a|a|4|d`).
- **Model Card `2|a`**: rebuilt 2-level `**结构**`, added `**分部**` (3 parts for 7 chapters: 第一部分 设定与一般筛选 `2|a|a` 起; 第二部分 线性均衡与规模分布 `2|a|a|3` 起; 第三部分 严格证明与扩展变体 `2|a|a|5` 起), expanded `**主要结果**` across the unified model, and wrapped `- 标签：` onto two lines (`R2` cleared).
- **Card-form & setting-tree hygiene (0 structural changes to `2|a|a` tree)**: cleared all 15 `R3` semicolon faults across `2|a|a|a`, `f`, `f|a`, `g`, `g|a`, `g|b`, `h|a`, `h|b`, `i`, `j`; fixed 3 corrupted-backslash LaTeX titles (`f|a`, `f|c`, `g|a`); updated `2|a|a|a` first-appearance links (`g_{S,N}` -> `4|f`; split `p(N,N'), p(N,A)` -> `4|b` and `g_N` -> `4|e`); replaced list numbering `（1）`–`（5）` on `2|a|a|i|b` to eliminate false `(4)`/`(5)` premise hits; removed inline `\tag {10}`/`\tag {11}` on `2|a|a|4|c|a..2` (`R22` ×5 cleared) while preserving every `$$...$$` display formula and `\tag{...}` byte-for-byte; anchored all 9 loose `局限`/`边界` cards (`loose` 9 -> 0).
- **`--premises` audit (17 hits, 0 hidden forward uses)**: 3 false-positive `F_\mu(0)=0` matches against `(0)` (`H|a|c`, `H|a|5|c`, `H|a|5|c|a`); 5 Appendix A setting/solution-concept pointers (`H|a|h|a`, `H|a|h|b` naming eqs. (4)–(7)); 6 chapter TOC pointers (`H|a|3`, `H|a|4`, `H|a|5`); 2 explicit Appendix A.5 external-proof pointers (`H|a|3|d` naming `(A.1)–(A.2)`); 1 prose remark pointer (`H|a|3|b` noting eq. (6) allows $\mu_H\le w$ when $Z$ is small).

### 5\|b (empirical, 30 parts)

- Added `**直觉**` leads (>= 20 chars) to all 4 main-line setup cards (`5|b|a` 数据, `5|b|a|1` 假说, `5|b|a|2` 设计, `5|b|a|3` 规格：基准), preserving all existing text, display formulas, artifact images, and links byte-for-byte.
- Fixed bare LaTeX `\hat{m}_j` -> `$\hat{m}_j$` on `5|b|a|6` (`R22` cleared).
- Anchored off-spine limits card `5|b|b` (`见设计 [[chatterjee2012spinoffs/5|b|a|2]]。`, `loose` 1 -> 0).

### Term relink & Gate results

- `zt-digest term-link -Step relink -CitationKey chatterjee2012spinoffs`: `cards_changed: 8, links_added: 23, links_repaired: 0, cards_refused: 0`; follow-up relink and `-PlanOnly` confirmed `cards_changed: 0, links_added: 0, links_repaired: 0`.
- `list_broken_links` (`agent`): `[]` (0 broken links).

| Model | Members (main/side/off) | `loose` | `--stage restructure` | `--network` | `--settings [--symbols]` | `--skip-shared` | `--premises` |
|---|---|---|---|---|---|---|---|
| `2\|a` (formal) | 117 (8/108/1) | 0 | pass (`self_sufficient: true`, `formal_structured: true`, rc=0) | `network: clean` (9 blocks, 2 layers, 3 parts, rc=0) | `all in the setting part` (rc=0) | `faults: 0` (rc=0) | 17 pointers / false-pos `(0)`, 0 hidden uses |
| `5\|b` (empirical) | 30 (7/22/1) | 0 | pass (`self_sufficient: true`, `evidence_bound: true`, rc=0) | `network: clean` (8 blocks, 5 layers, rc=0) | `clean` (rc=0) | `faults: 0` (rc=0) | `none` (rc=0) |

## Stage 4: imports and explanations

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; all writes serialized under `zt_mutex.py`). Pre-Stage-4 backup: `D:\zt\backups\zt-before-iof10-stage4-import-2026-11-27.sqlite3`.

### 2\|a (formal, 117 parts — lender)

- **Imports:** 0 imports added (`imports.mjs check`: exit 0; `imports.mjs plan`: `nothing to import`; `false_share: 0`, `tolerated: 0`). `2|a|a|4|h` (`校准：图 7 比较静态的规模上限与比值网格`) keeps its external SBA benchmark citation (`0.0736`) without importing `5|b|a|4|c`, avoiding an import cycle with borrower `5|b`.
- **Lending:** `2|a` now lends `2|a|a|4|d` (`整块`, 3 members + 14 closure cards = 17 shared cards) and `2|a|a|4|g` (`单卡`, 1 shared card) to `5|b`.
- **Explanations (step 8, `model-explain`):** `model-explain -Step plan -Force` emitted 1 job covering 3 Derivation parts (`2|a|a|4|c|a`, `2|a|a|4|c|a|1`, `2|a|a|4|c|a|2` — Proposition 3 proof chain refreshed after Stage 3 cleanup, including fixing a stale `第 1 章` reference in `4|c|a|2`). Written directly (no DeepSeek), self-checked with `zt-digest model -Step explain-check` (`faults: 0`), applied (`applied: 3, refused: 0, blocked: 0`), and finished (`left: 0, refused: 0, blocked: 0, link.second: 0`).

### 5\|b (empirical, 30 own + 18 shared = 48 members — borrower)

- **Premise wiring:**
  - `5|b|a|1` (`假说：尤尔分布与进入规模`): replaced `参照 [[chatterjee2012spinoffs/2|a|a|4]]` with `依赖 [[chatterjee2012spinoffs/2|a|a|4|d]]` (Corollary 1 & bounded-support rescaled Yule distribution $\tilde{\delta}_N$) and `依赖 [[chatterjee2012spinoffs/2|a|a|4|g]]` (Proposition 5 Pareto upper tail / thinner tails under truncation), clearing the baseline `advice (copy)` on `5|b|a|1 ~ 2|a|a|4`.
  - `5|b|a|3` (`规格：基准`): added `依赖 [[chatterjee2012spinoffs/2|a|a|4|d]]` because the baseline calibration specification directly uses Corollary 1 eq. (12) and its bounded-support rescaling $\tilde{\delta}_N$.
- **Imports (`imports.mjs plan` + `apply`, 2 rounds):**
  - Round 1: wrote `单卡 [[chatterjee2012spinoffs/2|a|a|4|d]]` and `单卡 [[chatterjee2012spinoffs/2|a|a|4|g]]` (`tolerated 0 -> 6`).
  - Round 2: upgraded `2|a|a|4|d` to `整块 [[chatterjee2012spinoffs/2|a|a|4|d]]` (members `4|d`, `4|d|a`, `4|d|b` + 14 closure cards: `4|c`, `4|b`, `g*`, `f*`, `a`–`e`; `tolerated 6 -> 0`), while keeping `2|a|a|4|g` as `单卡` (rested on only by `假说` `5|b|a|1`).
  - Round 3 `plan`: `nothing to import`. Polished both `**引入**` descriptions in `5|b` and noted `（经 **引入**）` in `**关联**`.
- **Symbols & Explanations:** `5|b` keeps its `变量表` (`5|b|a|a`); `zt-check --settings` clean. `model-explain -Step plan -Force` returned `nothing to explain` (`parts: 0, jobs: []`).

### Term relink & Final Stage 4 Gate (post term-relink)

- `zt-digest term-link -Step relink -CitationKey chatterjee2012spinoffs`: `cards_changed: 2, links_added: 3, links_repaired: 0, cards_refused: 0`; follow-up `-PlanOnly`: `cards_changed: 0, links_added: 0, links_repaired: 0`.
- `list_broken_links` (`agent`, `chatterjee2012spinoffs/`): `[]` (0 broken links).

| Model | Members (main/side/shared/off) | `imports check` | `--stage final` | `--settings [--symbols]` | `--network` | `--skip-shared` | `tolerated` / `false_share` |
|---|---|---|---|---|---|---|---|
| `2\|a` (formal) | 117 (8/108/0/1) | exit 0 | pass (`self_sufficient: true`, `formal_structured: true`, `loose: 0`, rc=0) | `all in the setting part` (rc=0) | `network: clean` (9 blocks, 2 layers, rc=0) | `faults: 0` (rc=0) | 0 / 0 |
| `5\|b` (empirical) | 48 (7/22/18/1) | exit 0 | pass (`self_sufficient: true`, `evidence_bound: true`, `loose: 0`, rc=0) | `clean` (rc=0) | `network: clean` (10 blocks, 5 layers, 2 `共用` shelves, rc=0) | `faults: 0` (rc=0) | 0 / 0 |

## Stage 5: evidence dialogue

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run). Backup: `D:\zt\backups\zt-before-iof10-stage5-dialogue-2026-11-27.sqlite3`.

- `dialogue.mjs survey 'chatterjee2012spinoffs/5|b'`: `ours: 6`, `theirs: 2729`, `pairs: 0` (0 candidate pairs; dialogue complete with no edits needed). (`2|a` is formal, so Stage 5 does not apply.)
- Gate: `dialogue.mjs check` not needed / clean (`0` pairs proposed).

## Stage 6: literature organize

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; all writes serialized under `zt_mutex.py`). Pre-Stage-6 backup: `D:\zt\backups\zt-before-iof10-stage6-organize-2026-11-27.sqlite3`. No DeepSeek calls ran.

### Source divisions

From `D:\zt-digest\chatterjee2012spinoffs\book.zh.md`: `1. 引言`, `2. 模型` (`§2` untitled opening, `2.1. 企业家的问题`, `2.2. 工人的问题`, `2.3. 均衡`), `3. 均衡刻画`, `4. 比较静态、经验含义与经验证据`, `5. 企业规模的不变分布：理论与证据`, `6. 结论`, `附录` (thin 1-sentence wrapper over `A. 证明` with `A.1`–`A.7` and `B. 允许企业家产生创意的模型扩展`), plus the Term list and Cited list at the tail.

### Final tree (top line, 10 Cards, 277 total unchanged)

```
1   1. 引言 (12)                                 1|c..1|c|8 §1 notes · 1|d structure Card
2   2. 模型 (131, rewrite)                       2|a formal model (117 parts) · 2|b §2 开篇 · 2|c 2.1 · 2|d 2.2 · 2|e 2.3 (from 3|b)
3   3. 均衡刻画 (9, rewrite)                     3|b..3|b|4 §3 notes (old 3|b|1..5)
4   4. 比较静态、经验含义与经验证据 (7)           4|a..4|a|5 §4 notes
5   5. 企业规模的不变分布 (52)                    5|b empirical model (30 own parts) · 5|c..5|c|12 §5 notes · 5|d–5|f 3 Findings
6   6. 结论 (5)                                  6|a..6|a|3 §6 notes
7   附录 A. 证明 (10, rewrite)                    7|d A.1 · 7|e A.2 · 7|f A.3 · 7|g A.4 · 7|h A.5 (2) · 7|i A.6 · 7|j A.7 (2)
8   附录 B. 允许企业家产生创意的模型扩展 (3)      old 7|d|9 + 8|a..8|a|1 (old 7|d|10..11)
9   术语表 (22, from 1|b)
10  参考文献 (25, from 1|a)
```

### Moves, crossers, and leads

- **3 atomic batches (`batch.mjs --commit`, tail-to-head, 0 breaks, 0 refusals)**:
  1. `01_appendix.ops` (8 moves, 11 renamed, 52 link rewrites): promoted Appendix B (`7|d|9..11`) out of the `7` Appendix wrapper to top Card `8` (`8` + `8|a..8|a|1`), and split Appendix A.1–A.7 (`7|d..7|d|8`) into 7 Side branches of `7` (`7|d`–`7|j`).
  2. `02_sec2_sec3.ops` (4 moves, 20 renamed, 46 link rewrites): detached the 11 Idea reading notes (`2|a|1..7` + 4 footnotes) from formal Model Card `2|a`'s Direct chain into `2`'s subdivisions in source order (`2|b` §2 opening, `2|c` §2.1, `2|d` §2.2), and moved `§2.3` (`3|b`, single card) from `3` to `2|e` while `3|b|1..5` (`§3`) closed up under `3`.
  3. `03_lists_to_tail.ops` (2 moves, 47 renamed, 764 link rewrites): moved the Term list (`1|b`) to `9` and the Cited list (`1|a`) to `10` (fixing their reversed order under `1`).
- **Whole units**: both Model Cards stayed intact at their exact original addresses — formal `chatterjee2012spinoffs/2|a` (117 parts; 129 -> 118 subtree cards due solely to the 11 detached Idea reading notes, 0 model parts moved) and empirical `chatterjee2012spinoffs/5|b` (30 own parts + 18 shared = 48 members; 31 subtree cards unchanged).
- **Summary leads & edits**: `leads.mjs --commit` rewrote `2` (`2. 模型`), `3` (`3. 均衡刻画`, removing stale `§2.3` text), and `7` (`附录 A. 证明`). Cleaned group-wrapper titles on `1`, `4`, `5`, `6`, `8`; added source lines to `1`, `4`; updated `6`'s position line to link both `7` and `8`; normalized Finding source lines on `5|d`–`5|f` to `来源：[[chatterjee2012spinoffs]] §5`; updated root index (`chatterjee2012spinoffs`) to the 10-Card top line and structure Card (`1|d`) to the 8 content sections and 2 surviving models.

### Gate results (post term-relink)

- `zt-digest term-link -Step relink -CitationKey chatterjee2012spinoffs` & `-PlanOnly`: `ok: true`, `cards_changed: 0`, `links_added: 0`, `links_repaired: 0`.
- `zt-check --cards` (root, `1`, `1|d`, `2`, `3`, `4`, `5`, `5|d`–`5|f`, `6`, `7`, `8`): `faults: 0` (RC=0).
- `zt-check --tree chatterjee2012spinoffs`: `no fault` (RC=0); `--snapshot` / `check.mjs`: 277 Cards unchanged, 0 broken links, only expected `2|a` 129 -> 118 reading-note detachment.
- `model-survey.mjs chatterjee2012spinoffs`: 2 Model Cards — `chatterjee2012spinoffs/2|a` (formal, 117 parts, `formal_structured=true`) and `chatterjee2012spinoffs/5|b` (empirical, 48 members = 30 own + 18 shared) (RC=0).
- `zt-check --model 'chatterjee2012spinoffs/2|a'`: `--stage final` (`pass`, RC=0), `--settings --symbols` (`all in the setting part`, RC=0), `--network` (`network: clean`, RC=0).
- `zt-check --model 'chatterjee2012spinoffs/5|b'`: `--stage final` (`pass`, RC=0), `--settings` (`clean`, RC=0), `--network` (`network: clean`, RC=0).
- `list_broken_links`: `total_broken: 0`, `chatterjee_broken: 0`.

## Stage 7: illustrate

Date: 2026-11-27, live box (`D:\zt`, user-authorized autonomous run; all writes serialized under `zt_mutex.py`). Pre-Stage-7 backup: `D:\zt\backups\zt-before-iof10-stage7-illustrate-2026-11-27.sqlite3`.

### Module & illustrated Cards (`2|a`, formal, 117 parts)

- **Module:** `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\chatterjee2012spinoffs-2_a.mjs` (`head = 'chatterjee2012spinoffs/2|a'`).
- **Model Card (`2|a`):** 3 `part(title, html)` subcharts in `**图**`:
  1. `一 · 博弈与筛选顺序` (4-stage single-period screening & spinoff order of moves with backward/equilibrium solution order).
  2. `二 · 因果结构` (5-layer SVG from exogenous primitives through market pricing $Z$, $\theta$ and worker option $f_0$, ex-ante screening $\mu_L,\mu_H$, ex-post retention $P_L,P_H$, to Yule distribution $\delta_N$, Gibrat's Law, and Pareto upper tail, with dashed market-clearing feedback loop).
  3. `三 · 变体与扩展对比` (4 clickable chapter panels: general bounded concave baseline & proofs, linear utility / Yule distribution / $\lambda_H/\lambda_L$ comparative statics, CARA risk-aversion variant, and entrepreneur-generated ideas + exogenous entry cost $f>0$ extension).
- **16 owned `设定`/`假设` Cards** (`2|a|a`, `b`, `c`, `d`, `f`, `f|a`, `f|b`, `g`, `g|a`, `g|b`, `h`, `h|a`, `i`, `i|a`, `j`, `j|a`; 0 `定义` parts in chapters):
  - Each received 1 economic-structure `chart` figure at the end of `**说明**` (zero card-tree maps, all plot axes named via `axes()`, all `data-zt` targets verified, `--zt-*` `TONE` tokens only) and a lint-clean 3-line `**直觉**` (`是什么`/`为什么需要`/`怎样进入模型`), clearing all 10 pre-existing `intuition-link` and `intuition-long` faults on `h`, `i`, `j`, `g|a`, `h|a`, `i|a`, `j|a`.

### Visual inspection & idempotence

- Rendered all 40 light and dark PNGs (including `-part1/2/3-*`) via headless Edge (`verify.mjs`). Visually inspected the Model Card parts and all 16 setting-card figures; fixed 11 layout/math details (`^{∞}` superscripts in `a`/`f`, nested subscript formatting in `c`/`h`/`i|a`, `, ` spacing in `μ_L, μ_H` on `2|a`/`g`, multi-line box text in `g`, curve/line label clearance in `b`/`c`/`f|a`/`f|b`/`i`, and middle-gap arrow / feedback-line clearance in `2|a` Part 2), re-applied under `zt_mutex.py`, and re-verified all 11 updated PNGs cleanly in both light and dark themes.
- Idempotence verified both before and after `term-link relink`: `apply.mjs` dry-run returned `same` for all 17 Cards.

### Term relink & Final Stage 7 Gates (post term-relink)

- `zt-digest term-link -Step relink -CitationKey chatterjee2012spinoffs`: `cards_changed: 11, links_added: 15, links_repaired: 0, cards_refused: 0`; follow-up `-PlanOnly`: `cards_changed: 0, links_added: 0, links_repaired: 0`.
- `zt-check --model 'chatterjee2012spinoffs/2|a' --charts`: **PASS** (RC=0) — `17 chart(s) in 118 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 card(s) without a full 直觉`.
- `node scripts/verify.mjs --model 'chatterjee2012spinoffs/2|a'`: **PASS** (RC=0) — `17 chart(s) in 118 card(s); 0 lint fault(s); 0 render failure(s); 0 card(s) without a figure; 0 card(s) without a full 直觉`.
- `zt-check --model 'chatterjee2012spinoffs/2|a' --settings --symbols`: **PASS** (RC=0) — `settings: all in the setting part`.
- `zt-check --model 'chatterjee2012spinoffs/2|a' --stage final`: **PASS** (RC=0) — `members 117 {"main":8,"off":1,"side":108}`, `loose 0; self_sufficient true; formal_structured true; stage(final) pass`.
- `zt-check --model 'chatterjee2012spinoffs/2|a' --network`: **PASS** (RC=0) — `network: clean` (9 blocks, 2 layers, 17 paths).
- `zt-check --model 'chatterjee2012spinoffs/2|a' --premises`: 17 pre-existing pointer / `F_\mu(0)=0` hits (unchanged from Stage 3; 0 new hits).
- `zt-check --model 'chatterjee2012spinoffs/2|a' --skip-shared`: **PASS** (RC=0) — `faults: 0`.
- `list_broken_links` (`agent`): **PASS** — `total_broken: 0`, `chatterjee_broken: []`.


