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
