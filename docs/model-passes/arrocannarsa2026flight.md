# arrocannarsa2026flight: post-digest model passes

Arro-Cannarsa (2026), *Flight to Safety: Monetary Policy in an Open Economy*, IMEN1 queue paper 5. Digest run wf_f3ec6ae3-25c plus mode term-link wf_0c70dc55-951 (2026-11-13, OpenCode Go), run report `complete: true`, 0 gaps. Post-digest sequence (`zt-post-digest`) on the live box at the owner's request, run by a side session of the IMEN1 queue (2026-11-14) while the queue digested other books on the same box. Stage subagents run on Opus at effort medium (owner).

Open owner decision carried from the digest: Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a record the same 2009-03-12 Swiss National Bank FX intervention. Merge them? This sequence does not decide it; the Result section gives their final addresses.

## Stage 1: fold

Date 2026-11-14, live box. Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 22 Model Cards (20 formal, 2 empirical), 16 candidate groups (G1: 2\|a, 3\|a, 4\|a, 5\|a, 6\|a, 7\|a, 11\|a by rests-on; every other card alone, most joined to G1 by **引入** imports).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 2\|a | §2.1–2.2 | two-country flight-to-safety model | 2\|a | formal; the general framework and the home country H (three assets, four agents, convenience demand); first formal card, kept as head |
| 3\|a | §2.3 | same | 2\|a | formal; the foreign country F of the same two-country model (household, two pricing conditions, government budget) |
| 4\|a | §2.4 | same | 2\|a | formal; the asset-market relations (six indifference conditions) of the same model; rests on 2\|a and 3\|a |
| 5\|a | §3 | same | 2\|a | formal; the equilibrium of the same model (market clearing, aggregate demand, current account) |
| 6\|a | §3.1 | same | 2\|a | formal; the same model log-linearized (13 equations) |
| 7\|a | §4.1 | same | 2\|a | formal; the steady state of the same model, solved recursively from the observed policy rate (calibration) |
| 11\|a | §4.5, §5.1 | same | 2\|a | formal; the baseline shock (AR(1) in θ*) and its impulse responses in the same linear system |
| 12\|a | §5.2 | same | 2\|a | formal; Proposition 1 (regime dependence on the steady-state convenience yield) of the same model |
| 13\|a | §5.3 | same | 2\|a | formal; a variant that changes one assumption (an occasionally binding zero lower bound) and keeps the rest |
| 14\|a | §5.4 | same | 2\|a | formal; a variant that changes one assumption (the central bank sets convenience-asset supply by a rule) |
| 15\|a | §5.5 | same | 2\|a | formal; a variant that changes one assumption (reserve-backed issuance) |
| 17\|a | §5.7 | same | 2\|a | formal; the permanent-flight experiment in the fixed-supply economy of the same model |
| 17\|b | §5.7 | same | 2\|a | formal; the known-duration flight experiment of the same model |
| 19\|a | A.1 | same | 2\|a | formal; derivation of the same model's expenditure minimization (price indices, demands) |
| 19\|b | A.2 | same | 2\|a | formal; derivation of the same model's Calvo pricing and Phillips curve |
| 20\|a | A.3 | same | 2\|a | formal; the same model's deterministic steady state in full |
| 21\|a | B | same | 2\|a | formal; the proofs of Propositions 1–4 of the same model and the kinked-rule threshold |
| 25\|a | C.5 | same | 2\|a | formal; the two-target balance-sheet experiment of the same model |
| 27\|a | C.7 | same | 2\|a | formal; the net resource transfer of the same model (intertemporal budget constraint) |
| 28\|a | C.8 | same | 2\|a | formal; the permanent and finite-duration flight experiments of the same model in full |
| 8\|a | §4.2 | convenience-yield measurement | 8\|a | empirical; measures the Swiss franc convenience yield (1994–2021 three-month rates, about 287 bp at end-2008) as a calibration input; an estimation is never folded into the theory it feeds (as velic2025inflation 7\|a) |
| 16\|a | §5.6 | acute-phase data comparison | 16\|a | empirical; sets six official Swiss statistics for 2008Q3–2009Q2 beside the model's four branches (Table 6): a test of the theory, a model of its own |

Three models. The evidence settled every row: every formal card shares the two countries, the three assets, the convenience demand and the policy rules, and each section solves, calibrates, proves or varies one assumption of that model.

### Fold (executed 2026-11-14, live box)

Method: the per-card fold script of the assenza2026louder pass (`fold.mjs`, with a batch retry), driven in book order by `drive.mjs` (this session's scratchpad), one Card per run: strip **结构**/**引入**, mask every Link to a Model Card or part, park the parts beside the section anchor, retype (facet `第 N 章：…`, emphasis core, level cleared), move the head `alone` to the end of the head's main line, return the parts, restore the body with the rename map. Idea-note Direct chains went to the section anchors (5\|a\|1–10, 6\|a\|1–10, 7\|a\|1–7, 13\|a\|1–7, 15\|a\|1–8, 19\|a\|1–3, 19\|b\|1–6, 20\|a\|1–5, 25\|a\|1–9, 28\|a\|1–8), as did the Idea Side cards 13\|a\|a and 28\|a\|a. Every **引入** line of the folded heads (4\|a, 5\|a, 6\|a, 7\|a, 11\|a and others) named a part that is now 2\|a's own, so none was appended. All 19 runs returned ok.

| Former head | Now | Facet |
|---|---|---|
| 3\|a | 2\|a\|a\|16 | 第 1 章：F 国（§2.3） |
| 4\|a | 2\|a\|a\|17 | 第 2 章：资产市场关系（§2.4） |
| 5\|a | 2\|a\|a\|18 | 第 3 章：均衡（§3） |
| 6\|a | 2\|a\|a\|19 | 第 4 章：对数线性化一般均衡（§3.1） |
| 7\|a | 2\|a\|a\|20 | 第 5 章：稳态识别（§4.1） |
| 11\|a | 2\|a\|a\|21 | 第 6 章：基准经济的外逃冲击（§4.5、§5.1） |
| 12\|a | 2\|a\|a\|22 | 第 7 章：便利收益的作用与区制依赖（§5.2） |
| 13\|a | 2\|a\|a\|23 | 第 8 章：零利率下限（§5.3） |
| 14\|a | 2\|a\|a\|24 | 第 9 章：资产负债表政策（§5.4） |
| 15\|a | 2\|a\|a\|25 | 第 10 章：储备支持型操作（§5.5） |
| 17\|a | 2\|a\|a\|26 | 第 11 章：永久性外逃（§5.7） |
| 17\|b | 2\|a\|a\|27 | 第 12 章：已知持续时间的外逃（§5.7） |
| 19\|a | 2\|a\|a\|28 | 第 13 章：支出最小化（A.1） |
| 19\|b | 2\|a\|a\|29 | 第 14 章：Calvo 定价与菲利普斯曲线（A.2） |
| 20\|a | 2\|a\|a\|30 | 第 15 章：确定性稳态（A.3） |
| 21\|a | 2\|a\|a\|31 | 第 16 章：命题的证明（B） |
| 25\|a | 2\|a\|a\|32 | 第 17 章：两目标的资产负债表实验（C.5） |
| 27\|a | 2\|a\|a\|33 | 第 18 章：净资源转移（C.7） |
| 28\|a | 2\|a\|a\|34 | 第 19 章：永久性与有限期外逃实验（C.8） |

2\|a's own H-country derivations stay the setting's Direct chain (2\|a\|a\|1–15); the restructure makes them chapters.

Gate:

- `model-survey.mjs arrocannarsa2026flight`: 3 Model Cards, exactly the decided heads: 2\|a (formal, 256 parts), 8\|a (empirical, 16), 16\|a (empirical, 40).
- `check-model.mjs 2|a`: 256 members (35 main, 221 side); every former head a part; no closure fault; 3 `forward` faults (chapter 16, 2\|a\|a\|31, names its own parts \|a\|6, \|a\|12, \|a\|20 with `依赖` lines) and 19 `unanchored` chapter cards, loose 2: left to the restructure.
- `check-model.mjs 8|a`: 16 members, loose 1, `self_sufficient` true. `16|a`: 40 members, loose 1, `self_sufficient` true.
- `list_broken_links` (agent): empty.
- Term relink: `{"rounds":1,"ok":true,"cards_changed":5,"links_added":6,"cards_refused":0,"links_repaired":0}`.

## Stage 2: settings (strict)

Date 2026-11-14, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The notation card of 2\|a was created first by `create_successor`. The moves and the three deletes ran as one `apply_batch` tree batch of 40 items (preview, then commit under the plan hash; 252 Cards renamed; `links_that_will_break` empty). Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the equation numbers (`\tag{…}` and `\quad (N)`) of the old and new bodies, and every written card was read back and compared with the intended text (0 differences). No display formula and no equation number changed; one display block was dropped as a byte-identical duplicate (the F-country preference formula, kept in `g|a`, dropped from `g`). No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 892 Cards to 890: one notation card was created and three restating setting cards were deleted.

### 2\|a (formal)

Every setting left the main line and the chapters. The seven settings on the setting card's own main line (former 2\|a\|a\|1–4, 11, 12, 15) moved into the setting part, so the eight H-country derivations now stand on the main line as 2\|a\|a\|1–8 and the chapters renumbered: 第 1 章 is now 2\|a\|a\|9, … 第 19 章 2\|a\|a\|27. A chapter setting moved by `move_span` alone, so the derivation chain that stood on its Direct line rose into the chapter's Side slot `a`.

```
2|a|a   设定：H 国的三类资产与四类主体                         原始设定 (17|a merged in)
├ a  符号表与读法 (new; blocks: 时间与资产, H 国家庭, 非贸易品企业, F 国与均衡, 稳态与线性化, 基准经济与变体, 推导中的辅助符号, 术语)
├ b  假设：外国政策债券不提供便利服务 (from 2|a|a|2)           ← 2|a|a
├ c  假设：外国政策债券的调整成本是二次式 (from 2|a|a|1)       ← 2|a|a
├ d  设定：家庭偏好与消费束 (1) (from 2|a|a|3)                ← 2|a|a, b
│    d|a 设定：家庭预算约束 (2) (from 2|a|a|4)                ← d, c
│    d|b 设定：期内支出最小化 (55) (from 28|a, A.1)           ← d
├ e  设定：非贸易品最终品生产者 (8)(9) (from 2|a|a|11)          ← 2|a|a
│    e|a 假设：Calvo 定价与利润最大化 (10) (from 2|a|a|12; 29|a|1 merged in)  ← e, d
├ f  设定：政府预算约束 (13) (from 2|a|a|15)                   ← 2|a|a, c
├ g  设定：F 国代表性家庭与外国政府 (from 16|a)                 ← 2|a|a
│    g|a 假设：外国偏好 (16|a|1);  g|b 设定：外国家庭预算约束 (14) (16|a|2);  g|c 设定：外国政府预算约束 (17) (16|a|5)
├ h  设定：两国经济的均衡 (from 18|a)                          ← d|a, e|a, f, g
│    h|a 设定：五个市场出清 (24)–(28) (18|a|1);  h|b 假设：一价定律 (18|a|2)
│    h|c 假设：两条无庞氏条件 (from 33|a|1, C.7)               ← h, h|b
├ i  设定：确定性稳态，以政策利率为锚 (from 20|a)              ← h
│    i|a 假设：政策利率取急性期均值 (20|a|1)
│    i|b 假设：稳态下偏好移动因子取 1 … (from 30|a|1, A.3)     ← i, c, h|b
├ j  设定：对数线性化，19 个内生变量与政策闭合 (from 19|a)     ← h, i
│    j|a 假设：外国通胀与政策利率为零，路径有界 (from 31|a|1, B)  ← j
├ k  设定：基准经济：外国被动，冲击是 θ* 的 AR(1) (from 21|a; 22|a merged in)  ← j, j|a
│    k|a 假设：校准 (21|a|1);  k|b 假设：供给固定、泰勒原理、一阶近似 (22|a|1)
├ l  设定：变体一：零利率下限 (from 23|a)                       变体一 ← k
│    l|a 设定：泰勒规则与 (54) 的下限约束 (23|a|1)             ← l, k|a
├ m  设定：变体二：央行把便利资产供给当作工具 (from 24|a)       变体二 ← k
│    m|a 假设：ψ>0、规则不变 (24|a|1);  m|b 假设：两目标 (from 32|a|1, C.5)
│    m|c 设定：变体二之一：储备支持型操作与返还融资 (from 25|a)  ← m, c
│        m|c|a 假设：逐期稳定通胀，规则不以头寸为条件 (25|a|1)
├ n  设定：变体三：带下限的规则与永久外逃 (from 26|a)           变体三 ← k, l|a
└ o  设定：变体四：已知持续时间的外逃 (from 27|a)               变体四 ← k, l|a
     o|a 假设：完全预见、互补条件、新息 (from 34|a|1, C.8)      ← o, k
```

- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**. The symbol tables and term tables of the folded setting cards went into the notation card (64 rows, 28 terms), which covers every symbol of every setting and the auxiliary symbols the derivations introduce with 记 ($\lambda_t$, $X_k$, $L^*_t$, $B_t$, $V_q(q^*)$, $A_1$, $\kappa$, $\Phi_0$). Rows are written by the letter the check reads; a letter with two meanings in the source gets one row that names both ($B$, $\theta$, $\kappa$, $\lambda$, $T$, $N$, $s$). Each row names the card where a reader meets the symbol first, as the check reads it.
- Deleted `into` the card each restates (Links re-pointed): 17\|a (chapter 2: the two countries' households and the three assets) into the setting card 2\|a\|a, after its timing sentence (households observe the state, then choose; $S_{t+1}$ realizes next period) moved into item 2 and the $\tilde E_t$ remark into **说明**; 22\|a (chapter 7: the linearized small economy) into `k`, after its fixed supply moved into item 3 and its remark that Proposition 1 takes the paths as given into **说明**; 29\|a\|1 (A.2: reset probability, ownership, indexation) into `e|a`, whose items 2–4 now carry the exogenous reset probability and the indexation rule $P_{N,t}(i)=\Pi P_{N,t-1}(i)$. None of the three held a display formula.
- Retyped and kept in their chapters, because each holds display formulas found in no setting card (the appendix restatements with their own `\tag` numbers or spacing): 2\|a\|a\|22\|a `前提：附录 A.2 的厂商问题 (65)–(68)`, 23\|a `前提：稳态要用的均衡条件` ((25)–(38)), 24\|a `前提：证明用的两国线性化系统与政策规则` ((31), the (39)–(49) table, the two rules), 25\|a `前提：固定供给基准下的线性系统 (C.5)` ((39)–(49)), 26\|a `前提：可贸易品块与净流入 (C.7)` ((36)–(38), (84), (85), (51)), 27\|a `前提：两个外逃实验的系统 (C.8)` (no display; the figure 22 and the experiment parameters). Each lost its symbol and term tables and opens with a line naming the settings it restates.
- Statements moved and pointers: the setting card names its block order in **说明**; the variants `l`, `m`, `n`, `o` open with a `变体` line; `m|c` is the sub-variant of `m` (its `变体` line carries no Link, so the rests-on relation to `m` comes from its `依赖` line). The assumptions that rested on the chapter setting cards now rest on the setting they refine (`i|b` on `i`, `j|a` on `j`, `m|b` on `m`, `h|c` on `h`, `o|a` on `o`). `i|b` and `h|c` cite the one-price assumption `h|b` by Link instead of restating it. `l` no longer rests on `l|a` (an `order` fault): the rule is named as the next setting card in **直觉**.
- Definitions added at first use: $S_t$, $V(\cdot)$, $q_t$ (look-up), $Q^s_t$, $F^s_t$ in the setting card; $P_{T,t}$, $\bar F$, $P_T^*$ in `c`; $C_{T,t}$, $C_{N,t}$ in `d`; $Y_{N,t}$, $P_{N,t}(i)$ in `e`; $A_t$, $H_t(i)$ in `e|a`; $F^*_t$, $Q^*_t$, $P^*_{T,t}$ in `g`; $q^*_t$, $f_t$, $f^g_t$ in `h|c`; $\varepsilon_t$ in `o|a`. The linearization coefficients point to the definition card 2\|a\|a\|12\|a.
- The **结构** section of 2\|a was rebuilt from the build order (setting part and main line).

### 8\|a (empirical)

- The measure 8\|a\|a\|b now has a one-line summary, a **起点** section with its `依赖` line and the former `见` pointer, and a **内容** section.
- Each of the three specifications already had its equation. Their **估计式** sections now open with `原书没有写出本规格的估计式。本卡按……补写：`, followed by the existing equation; the internal file reference (`s09.md 第 5、13 行`) became `§4.2 正文`. No equation changed and none was added.

### 16\|a (empirical)

- The data tree and the specification ladder were already in place, and each of the five specifications already states its equation with the 补写 opening ($x^{data}_{k}=Z_{k}$; $x^{b}_{k}=\mathcal{M}^{b}_{k}(\varepsilon)$ for the four model branches). Each variant now names, after its equation, which branch the superscript $b$ takes. No equation changed. There is no IV specification in 8\|a or 16\|a.
- The Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a were not touched.

### Gate

- `check-settings.mjs arrocannarsa2026flight/2|a`: `settings: all in the setting part, as a dependency tree` (advice only: the setting card has 14 settings filed under it).
- `check-settings.mjs arrocannarsa2026flight/8|a`: `settings: the data tree and the specification ladder are in place`.
- `check-settings.mjs arrocannarsa2026flight/16|a`: `settings: the data tree and the specification ladder are in place`.
- `check-model.mjs 2|a`: 254 members (28 main, 226 side), loose 2, no closure fault; 3 `forward` faults (chapter 16, 2\|a\|a\|24, names its own parts \|a\|5, \|a\|11, \|a\|19 with `依赖` lines) and `unanchored` on 18 chapter cards (2\|a\|a\|9–23, 25–27), as after stage 1; left to the restructure.
- `check-model.mjs 8|a`: 16 members, loose 1, `self_sufficient` true. `16|a`: 40 members, loose 1, `self_sufficient` true.
- `list_broken_links` (agent): empty, before and after the relink.
- Term relink: `{"cards_changed":33,"ok":true,"cards_refused":0,"applied":true,"links_added":105,"book":"arrocannarsa2026flight","rounds":1,"links_repaired":0}`. The three settings checks and `list_broken_links` were rerun after it and stay clean.

### Open

1. 2\|a: 18 chapter cards are unanchored and chapter 16 has three forward faults; the eight H-country derivations (2\|a\|a\|1–8) stand on the main line before chapter 1. The restructure builds the spine. The Model Card's **概要** still says that market clearing and the F country are in later models; the restructure rewrites it.
2. The setting card carries fourteen settings (advice `wide`). The blocks are named in its **说明**; no block card was added.
3. The six `前提` cards in chapters 14–19 restate settings with the appendix's own equation numbers. The restructure decides whether they stay as chapter heads or become result cards; deleting them would drop display formulas that stand nowhere else byte for byte.
4. The check reads some symbols first in a card that only carries them in a sub- or superscript ($N$ in $C_{N,t}$ in `d`, $s$ in $Q^s_t$ in the setting card); the rows name both readings.
5. The owner decision on merging the Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a stays open.

## Stage 3: restructure

Date 2026-11-14, live box, under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; the chapter leads were written by hand). Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Moves ran as six `apply_batch` tree batches (2\|a: one batch of 13 cross-chapter items, three chain-cutting batches of 27, 37 and 36 items, one batch of 1 item; 8\|a and 16\|a: one batch of 2 items), each previewed, then committed under the plan hash; `links_that_will_break` was empty in every preview. The first commit of the 27-item batch returned no reply; the script previewed again and committed, and the tree read afterwards shows the batch applied once. Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the equation numbers of the old and new bodies, and after the pass a script compared the whole book with the backup: 1,390 display formulas and 227 `\tag`/`\quad (N)` equation numbers before and after, no difference. No `card_locked` error and no `read_view_changed` error occurred. The Literature tree went from 890 to 892 Cards (two new chapter cards, 2\|a\|a\|1 and 2\|a\|a\|2); no Card was deleted. The Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a were not touched.

### 2\|a (formal): the intuition spine

```
2|a|a  设定 (setting part as the settings pass left it)
2|a|a|1   第 1 章：H 国家庭与三类资产的定价（§2.2） (new)       ← 2|a|a|d, d|a
    a 结果：价格指数与支出份额 (3)(4)        (proof a|a 证明：支出最小化（A.1）, the former chapter 13, with its chain a|a|a…a|a|a|7)
    b 结果：便利收益形成回报楔子 (5)–(7)     (proof b|a 一阶条件 → (5) → (6) → (7), the former main-line cards 2–5)
2|a|a|2   第 2 章：H 国厂商的 Calvo 定价（§2.2） (new)          ← 2|a|a|e, e|a
    a 推导：最优重设价格 (11) → a|1 价格指数的演化 (12)
2|a|a|3   第 3 章：F 国（§2.3）                                  ← 2|a|a|g, g|b
    a 结果：外国持有者按 L* 折价持有         (proof a|a (15) → (16))
2|a|a|4   第 4 章：资产市场关系（§2.4）                          ← 第 1 章, 第 3 章
    a 结果：UIP 不成立                        (proof a|a (18) → … → (23), six steps)
    b 结果：两国便利收益之差;  c 附注
2|a|a|5   第 5 章：均衡（§3）                                    ← 第 1、2、3 章, 2|a|a|h
    a 结果：本国均衡条件                      (proof a|a (29) → … → (33)–(35))
    b 结果：外逃的进入与吸收                  (proof b|a (36) → (37) → 名义经常账户 → (38))
2|a|a|6   第 6 章：对数线性化一般均衡（§3.1）                    ← 第 5 章, 2|a|a|j
    a 定义：线性化系数
    b 结果：价格与加总的线性关系 (44)–(51)   (proof b|a, four steps)
    c 结果：17 个方程闭合系统                 (proof c|a (39) → (40) → (41) → (42) → (43))
    d 前提：证明用的两国线性化系统与政策规则（附录 B）
2|a|a|7   第 7 章：稳态识别（§4.1）                              ← 第 5 章, 2|a|a|i
    a 定义：便利收益楔子 (52);  b 结果：稳态通胀与实际利率 (proof b|a three steps; b|b 例子)
2|a|a|8   第 8 章：基准经济的外逃冲击（§4.5、§5.1）              ← 第 6 章, 2|a|a|k
    a 避风港模式;  b 三条吸收渠道;  c 紧缩冲击;  d 净资源转移为正 (proof d|a)
2|a|a|9   第 9 章：便利收益的作用与区制依赖（§5.2）              ← 第 6 章, 2|a|a|k
    a 结果：命题 1  (proof a|a 正文三步;  proof a|b 附录 B 四步 → 结论：命题 1（附录 B）)
    b 结果：命题 1 (i);  c 结果：命题 1 (ii) (c|a 例子)
2|a|a|10  第 10 章：零利率下限（§5.3）                           ← 第 8 章, 2|a|a|l
    a 方法;  b–e 四个结果;  f 限制
2|a|a|11  第 11 章：资产负债表政策（§5.4）                       ← 第 6 章, 2|a|a|m
    a 结果：命题 2  (proof a|a 正文四步;  proof a|b 附录 B 七步 → 结论：命题 2（附录 B）;  a|c 变体;  a|d 例子)
    b 结果：均衡存在且唯一 (proof b|a);  c 盯住汇率的代价;  d 加权目标的权衡 (proof d|a)
2|a|a|12  第 12 章：储备支持型操作（§5.5）                       ← 第 11 章, 2|a|a|m|c
    a 结果：命题 3  (proof a|a 正文两步;  proof a|b 附录 B 两步 → 结论：命题 3（附录 B）)
    b 稳定相同、对外账户不同 (b|a 例子);  c ψ>0 时等价破裂 (c|a 例子)
2|a|a|13  第 13 章：永久性外逃（§5.7）                           ← 第 6 章, 2|a|a|n
    a 结果：命题 4 (i)   (proof a|a 正文四步;  proof a|b 附录 B 四步 → 结论：命题 4 (i)（附录 B）;  a|c 例子)
    b 命题 4 (ii) (proof b|a);  c 命题 4 (iii) (proof c|a; c|b 例子)
    d 结果：命题 4 (iv)  (proof d|a 正文;  d|b 证明：命题 4 (ii)–(iv)（附录 B）, the former chapter 16 card, with its chain of four steps)
    e 命题 4 (v) (proof e|a)
2|a|a|14  第 14 章：已知持续时间的外逃（§5.7）                   ← 2|a|a|o, o|a
    a 方法;  b–e 四个结果 (b with proof b|a and example b|b)
2|a|a|15  第 15 章：Calvo 定价与菲利普斯曲线（A.2）              ← 2|a|a|e, e|a
    a 前提 (65)–(68);  b 重设价格比 (69);  c 价格指数 (71) 与 (70);  d 递归形式 (77)   (each with its proof chain)
2|a|a|16  第 16 章：确定性稳态（A.3）                            ← 2|a|a|i, i|b
    a 前提;  b 稳态便利收益楔子;  c 两国贸易品通胀相等;  d 外国持有量与净出口 (d|b 例子);  e 价格与资产市场闭合
2|a|a|17  第 17 章：两目标的资产负债表实验（C.5）                ← 2|a|a|m, m|b
    a 前提;  b 国内目标的操作规模 (b|b 图 18);  c 汇率目标的操作规模 (c|b 图 19);  d 加权目标的单调性
2|a|a|18  第 18 章：净资源转移（C.7）                            ← 第 5 章, 2|a|a|h|c
    a 前提;  b 跨期预算约束 (87);  c 净流入的流量形式 (88) (c|b, c|c 例子);  d 国外收入只取决于合并头寸
2|a|a|19  第 19 章：永久性与有限期外逃实验（C.8）                ← 第 13 章, 2|a|a|o
    a 前提;  b 永久性外逃的稳态 (b|b 图 22);  c 有限期外逃的衰退加速 (c|b 图 23, c|c 例子);  d 逼近命题 4 的阈值
```

- The eight H-country cards (former 2\|a\|a\|1–8) left the main line into two new chapter cards: 第 1 章 holds the result (3)(4) and the wedge (5)–(7) with its four-step proof chain; 第 2 章 holds the Calvo derivation chain (11) → (12), which ends in no result card (advice only).
- Appendices. A.1 (former 第 13 章) only proves (3)(4): its chapter card is now the proof branch `2|a|a|1|a|a` of that result (facet `证明：支出最小化（A.1）`, `本模型` → `本证明`), and the result card's sentence `本模型不重推` now points to it. A.2 and A.3 derive more than one result (the recursive Phillips curve (74)(76)(77); the steady state (78)–(83)) and stay chapters (第 15、16 章). Appendix B (former 第 16 章) was split by the results it proves: the 前提 card (the proof system) went to 第 6 章 (`6|d`), because every B proof starts from it; the proofs of 命题 1, 2, 3 and 4 (i) became the second proof branch (`…|b`) of 9\|a, 11\|a, 12\|a and 13\|a, and the former chapter card with the (ii)–(iv) chain became the proof branch 13\|d\|b of 命题 4 (iv) (facet `证明：命题 4 (ii)–(iv)（附录 B）`, one opening paragraph that names where the other three B proofs now stand). Its three `依赖` Links (the stage 1–2 forward faults) are unchanged: two now point back to the B proof of 命题 2 in 第 11 章, one into its own branch. Each B chain's last card has the facet `结论：命题 N（附录 B）`. C.5, C.7 and C.8 stay chapters (each has its own results).
- Decision on the six `前提` cards: all stay, each as the first Side card of its chapter (15\|a, 16\|a, 17\|a, 18\|a, 19\|a), and the B one as `6|d`. Each restates settings with the appendix's own equation numbers and holds display formulas found in no setting card; its chapter's proof chains rest on it.
- In every chapter the flat chain was cut into results with their proof chains as Side branches (a 定义, 方法 or 前提 card heading a chain stays the chapter's first Side card). Result facets now read `结果：…` (56 Cards; the former `结果` and `命题` facets; titles unchanged).
- Chapter renumbering, old → new: 1–12 → 3–14, 13 → (A.1 under 第 1 章), 14 → 15, 15 → 16, 16 → (B, split), 17–19 unchanged. Chapter facets carry the new numbers; chapter titles are unchanged. Chapter numbers in text were updated in the setting part (the setting card, the notation card, `d|b`, `h`, `i`, `i|a`, `i|b`, `j`, `j|a`, `k`, `k|b`, `l`, `l|a`, `m`, `m|c`, `n`, `o`; `d|b` and `j|a` now name the A.1 and B proofs) and in chapters 4 and 6. Two lines that say `第 4 章的校准` (6\|a and 6\|c\|a\|4) refer to the paper's §4 and were left.
- Every chapter card got **直觉** (lead of 20+ characters), **本章结果** (pointers `结果：`, `定义：`, `前提：`, `方法：`, `推导：`, `旁注：`; for 命题 1–4 the line names both proof branches) and **承上启下** with two to four `依赖` lines to the setting or earlier chapter it builds on; the old text follows (`本模型` → `本章`). This anchored all 19 chapters. 第 6 章 keeps the old **分部** section from its fold (it is not read from a chapter card).
- With the B proofs and the A.1 proof now ahead of the main-text derivations, `check-settings` found four symbols first met earlier: the notation rows of $\lambda_t$ (now naming both meanings, the A.1 multiplier included), $V_q(q^*)$, $A_1$ and $\Phi_0$ point to 1\|a\|a\|a, 1\|b, 11\|a\|b and 13\|a\|b\|2; 1\|b, 11\|a\|b and 13\|a\|b\|2 got one defining clause each.
- Hidden forward fault fixed by moving: the A.1 figure example (former 21\|a\|2\|a, `按 (58) 与 (62) 画图`) drew (62) before the card that derives it; it moved from under the relative-demand step to under the cost-share result (`2|a|a|1|a|a|a|4|a`).
- The Model Card: title and 问题 unchanged; 概要 rewritten for the whole model (the stale sentence that market clearing and the F country are in later models is gone); **结构** rebuilt from the tree; **分部** added (设定与两国模型 from 2\|a\|a; 均衡、线性化与校准 from 第 5 章; 外逃冲击与政策 from 第 8 章; 附录推导与实验 from 第 15 章). Its six stale 对应/对照 lines to its own chapters were dropped; the source line now names §2–§5 and appendices A–C.

Network (`check-network.mjs`): 设定 → {1, 2, 3, 5, 9, 10, 12, 14, 15, 16, 17, 18, 19}; 1 → 4; 5 → {6, 7}; 6 → {8, 11, 13}.

### 8\|a (empirical): the evidence spine

Main line unchanged: 数据 `8|a|a` → 设计 `|1` → 规格：基准 `|2` → 第 1 章 主要估计 `|3`. The paper has no hypothesis here (measurement). **直觉** leads added to the data, design and specification cards. The limits card 序列只确定量级 (former 8\|a\|b) moved under chapter 1 (`8|a|a|3|e`) with one reference line; loose 1 → 0. **结构** rebuilt.

### 16\|a (empirical): the evidence spine

Main line unchanged: 数据 → 假说：下限分支 `|1` → 假说：储备支持型 `|2` → 设计 `|3` → 规格：基准 `|4` → 第 1 章 主要估计 `|5` → 第 2 章 识别检验 `|6` → 第 3 章 机制 `|7`. The mechanism chapter was numbered 第 4 章 (no 第 3 章 existed); facet and title now read 第 3 章. **直觉** leads added to the data, the two hypotheses, the design and the specification. The limits card 窗口与单一冲击 (former 16\|a\|b) moved under chapter 1 (`16|a|a|5|k`, it limits the reading of table 6) with one reference line; loose 1 → 0. **结构** rebuilt.

### Gate

- `check-model.mjs 2|a`: 256 members (20 main, 236 side), loose 0, no closure, forward or unanchored fault, `self_sufficient` true (was false). `8|a`: 16 members (4 main, 12 side), loose 0, `self_sufficient` true. `16|a`: 40 members (8 main, 32 side), loose 0, `self_sufficient` true.
- `check-network.mjs`: `network: clean` for all three (advice only: 第 2 章 of 2\|a has no `结果：` card; it is one derivation chain).
- `check-premises.mjs`: 8\|a and 16\|a `premises: none used before their card`. 2\|a 39 hits, all pointers, kept with this reason:
  - 29 are setting-part **直觉**/**说明**/`怎样进入模型` lines (`c`, `e`, `e|a`, `i|b`, `j`, `j|a`, `k|b`, `l`, `m`, `m|a`) that say where (2), (7), (30)–(32), (36)–(44), (46), (49), (51), (65)–(68), (79), (82) are derived or used, as after stage 2.
  - `4|a|a|1` (4 hits) is the 思路 roadmap of its own chain (`全链依次推出` (20)–(23)); `15|b|a|1` names (71) as the next step's target; `1|a|a|a|5` remarks how (64) differs from the next step.
  - `6|a` (2) says the coefficients it defines enter (39)–(43).
  - `7|b|a|2` derives $\Pi_N=\beta(1+i)$ itself and names it by its appendix number (78).
  - `8|d|a` names (84) but writes the definition of $\mathcal N_t$ out in its own step 1.
- `check-settings.mjs`: 2\|a `settings: all in the setting part, as a dependency tree` (advice: 14 settings under the setting card); 8\|a and 16\|a `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` (agent): `{"broken":[]}`. Literature tree 890 → 892 Cards.
- Term relink: `{"links_repaired":0,"rounds":1,"applied":true,"links_added":143,"book":"arrocannarsa2026flight","cards_refused":0,"cards_changed":29,"ok":true}`. All checks above, `list_broken_links` and the formula comparison with the backup were rerun after it and stay as listed.

### Open

1. Step 3 card text was not redrafted: result cards keep their digest body (no `**为什么**`/`**证明**` sections), derivation cards keep their digest format, and the old chapter cards keep their 问题/概要/关联 text below the new sections; their titles still read `… 模型：…`.
2. The B proofs of 命题 1–4 and the A.1 proof restate the main-text derivations step for step (9\|a\|a against 9\|a\|b; 11\|a\|a against 11\|a\|b; 12\|a\|a against 12\|a\|b; 13\|a\|a against 13\|a\|b; 1\|a\|a against the result (3)(4)). Both proofs are kept; the import stage may decide whether one should cite the other.
3. 第 6 章 (2\|a\|a\|6) still carries the **分部** section the fold left in it (stale addresses into the setting part); 6\|a and 6\|c\|a\|4 say `第 4 章的校准` for the paper's §4.
4. The setting card still carries fourteen settings (advice `wide`, stage 2 open item 2).
5. The owner decision on merging the Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a stays open.
6. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.

## Stage 4: imports and explanations

Date 2026-11-14, live box, under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent; one DeepSeek job, for the explanation refresh). Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-imports.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok; drive D had 237 GB free). Text ran as `edit_card` calls (arrocannarsa2026flight Cards only, 5 Cards written by hand: 16\|a\|a\|1, 16\|a\|a\|2, 8\|a\|a\|b and the Model Cards 8\|a, 16\|a for the import-line wording). The Model Card rewording ran through a script that compared the display formulas and the backslash count before the write and read each Card back (`same`, backslashes 5 → 5 on 8\|a). After the pass a script compared the whole book with the backup: 891 Cards before and after, 1,390 display formulas identical, every `估计` value line identical; the only equation-number differences are `\quad (N)` references in the refreshed 思路/说明/解读 prose of the 19 explained parts, outside any display formula. A scan of every changed Card found no control character and no TeX command without its backslash. No `card_locked`, no `read_view_changed` and no failed batch occurred. Order: 2\|a, then 8\|a and 16\|a.

Before any edit, `check` exited 0 and `plan` said `nothing to import` on all three heads: the two empirical models named the theory only with `参照` Links (16\|a\|a\|1 → chapter 10, 16\|a\|a\|2 → chapter 12) or in words (8\|a\|a\|b `按 (52) 式`).

### 2\|a (formal)

- No import. The model reads in itself; no part rests on another model's part. Tolerated 0, false shares 0. It now lends the definition block `2|a|a|7|a` to 8\|a and four result Cards to 16\|a (`lent`).
- Cross-book copy advice judged by hand, both comparisons, not copies: `2|a|a|16` (第 16 章, the deterministic steady state, A.3) ~ gomezcram2026government/3\|i\|a\|5 (that model's linearized system and determinacy regions), joined only by the `跨书关联：对应` line `两处都在确定性稳态附近作一阶线性化`; `2|a|a|13|d|b` (the appendix B overview) ~ gomezcram2026government/3\|i\|a\|8 (A.2, regime loadings), joined only by `对应 … 两处都在附录里把线性化系统写成载荷递推`. The two models share a method, not a block: neither restates the other's setting, symbols or result. The advice stays in `check` output.
- Stage-3 open item 2 (the B proofs of 命题 1–4 restate the main-text derivations step for step; the A.1 proof restates the derivation of (3)(4)): both proofs lie inside 2\|a, and an import from the model's own range is refused (`import_target`), so no import applies. `check` lists no copy for these pairs (it compares across models only). Decision: both proofs stay; the paper gives both, and each is filed as its own proof branch of the result. Whether one should cite the other step by step is a restructure matter, left open.

### 8\|a (empirical: the convenience-yield measurement)

- Decision: the measurement imports the theory block that defines the convenience yield; 2\|a's calibration does not import 8\|a's estimate. Reasons: (1) the measure `8|a|a|b` takes its object from the definition (52) (`作者按 (52) 式把两者相减`), so the definition is its premise, the case of a measure resting on the theory it measures; (2) 2\|a holds no parameter card for $\nu$: the model treats $\nu$ as a parameter ($0<\nu<1$), its propositions hold for any $\nu$, and the value 300 bp enters only as the author's rounded choice in chapter 7's summary and the examples, with a plain pointer `便利收益的测量见 [[8|a]]`; importing `8|a|a|3|d` would need a new setting card in a model whose settings are closed, and would make the two models rest on each other. The pointer in chapter 7 stays a reference.
- `8|a|a|b` **起点**: `依赖 [[2|a|a|7|a]]：测量的对象是这张卡的定义 (52)，便利收益是两种债券利率之差。` (the duplicate `见 [[8|a|a]]` line dropped; the display formula unchanged).
- Import: `plan` round 1 `单卡 [[2|a|a|7|a]]` (tolerated 0 → 1, `2|a|a|i`), round 2 upgraded it: `整块 [[arrocannarsa2026flight/2|a|a|7|a]]：测量对象的定义，便利收益 $\nu$ 是普通债券利率与政策利率之差 (52)，利差序列是它的观测对应物` (tolerated 1 → 0). The closure brings 11 setting Cards of 2\|a (`i`, `h`, `d|a`, `e|a`, `f`, `g`, `d`, `c`, `e`, the setting card, `b`): 28 members (16 own + 12 shared).
- Symbols (step 6): the 变量表 `8|a|a|a` defines its observed $\nu_t$; `check-settings` finds no symbol uncovered, so no `沿用` line was added. No false `variable_uncovered` fault.

### 16\|a (empirical: Table 6 against the four branches)

- A test of the theory imports the predictions it tests, `单卡`. The two hypotheses now rest on the result Cards (the `参照` lines to the chapter cards were replaced; the `直觉` and prediction sentences unchanged):
  - `16|a|a|1` (下限分支): `依赖 [[2|a|a|10|b]]`（政策利率在下限停留十三个季度）, `依赖 [[2|a|a|10|c]]`（非贸易产出下降 3.1%，非贸易品通胀下降 3.3 个百分点）, `依赖 [[2|a|a|10|d]]`（冲击当期升值 3.9%）, each `本假说检验的预测之…`.
  - `16|a|a|2` (储备支持型): `依赖 [[2|a|a|12|b]]：本假说检验的预测，储备支持下政府外国资产上升，返还下政府外国资产不变。`
- `plan` → `apply` wrote four `**引入**` lines (made before **结构**), then reworded to say what the test uses:
  - `单卡 [[arrocannarsa2026flight/2|a|a|10|b]]：下限分支的预测之一，政策利率在下限停留十三个季度，对照表 6 的利率行`
  - `单卡 [[arrocannarsa2026flight/2|a|a|10|c]]：下限分支的预测之二，非贸易产出与非贸易品通胀的下降，对照表 6 的产出行与通胀行`
  - `单卡 [[arrocannarsa2026flight/2|a|a|10|d]]：下限分支的预测之三，冲击当期的升值，对照表 6 的升值行`
  - `单卡 [[arrocannarsa2026flight/2|a|a|12|b]]：储备支持型分支的预测，只有储备支持下政府外国资产上升，对照表 6 的央行外币投资行`
- They stay `单卡`; `plan` proposes no upgrade. Tolerated 0 → 2 (2\|a\|a\|10\|a, the solution method, and 2\|a\|a\|12\|a, 命题 3, stay in 2\|a), as the skill expects for a tested prediction. False shares 0. 44 members (40 own + 4 shared).
- The copy advice `16|a|a|1` ~ `2|a|a|10` and `16|a|a|2` ~ `2|a|a|12` came from the `参照` lines and is gone. The Model Card's **关联** Links (`检验 [[13]]`, `对照 [[15]]`, to Literature section cards) and the cross-book `对照 [[becker2025division/5|b|b|8]]` are no false shares; they stay for the evidence dialogue.

### Cross-book blocks

`plan` proposed no cross-book line for any head, and no part of the three models rests on another book's Card. The cross-book Links (gomezcram2026government on chapters 16 and 13\|d\|b, assenza2026louder and velic2025inflation on chapter 10, atkinson2015lectures and bergin2026exchange on chapter 12, atkinson2015lectures on chapter 7, becker2025division on 16\|a) are comparisons (`对应`, `对照`), kept.

### Explanation refresh (step 8)

- `explain.mjs plan`: 2\|a 19 Derivation parts in 1 job; 8\|a and 16\|a `nothing to explain`.
- One DeepSeek conversation (`opencode-go/deepseek-v4.1-flash`, effort high, the box's drive model; OpenCode Go account Primary, 24 requests, 434 s, 1 turn, context 143k of 1M; tools: 21 pwsh, 3 read, 2 write of which 1 failed and was retried; only `out/explain-01.json` was written in the work folder): answer `{"done":19,"blocked":0}`. Cost: OpenCode Go subscription, the Harness ledger books 0 CNY for the source (the 2026-11-14 opencode-go day line went from 3,296 to 3,370 calls during the job but is shared with the IMEN1 queue, so no per-job token split). Conversation a27aade5 deleted, workspace removed. Conversation 3b1d187b was not touched.
- `apply`: `applied 19, refused 0, blocked 0, follow-up jobs 0`. `finish`: `explained 19 part(s); Term links changed 19 Card(s), second plan 0; left 0; refused 0` (exit 0); parts 2\|a\|a\|4\|a\|a and \|1–\|5 (UIP chain (18)–(23)), 5\|a\|a and \|1–\|4 (home equilibrium (29)–(35)), 5\|b\|a and \|1–\|3 (flight entry and absorption (36)–(38)), 6\|b\|a and \|1–\|3 (linear price and aggregation relations (44)–(51)).
- Assumption Links: the refreshed text of the 19 parts names no `假设` and no (H\_n) label, and `check-settings` reports no `cite` fault, so no Link had to be added by hand. The stage-3 roadmap hits of `4|a|a|1` (4 `check-premises` hits) went with the refresh: 39 → 35 hits.

### Gate

- `imports.mjs check`: exit 0 on 2\|a, 8\|a, 16\|a; `plan`: `nothing to import` on all three (2\|a keeps the two cross-book copy advice lines above).
- `get_model`: `self_sufficient: true` on all three; false shares 0 everywhere; tolerated 0 on 2\|a and 8\|a, 2 on 16\|a (above); 8\|a imports `整块 2|a|a|7|a` (12 shared members), 16\|a four `单卡` lines; 2\|a lends to both.
- `explain.mjs finish`: exit 0 on all three (`left 0; refused 0`; 8\|a and 16\|a `explained 0 part(s)`).
- `check-model.mjs`: 2\|a 256 members `{"main":20,"side":236}`, loose 0, self_sufficient true; 8\|a 28 `{"shared":12,"main":4,"side":12}`, loose 0, self_sufficient true; 16\|a 44 `{"main":8,"side":32,"shared":4}`, loose 0, self_sufficient true.
- `check-network.mjs`: `network: clean` on all three (advice only: 第 2 章 of 2\|a has no `结果：` card, as at stage 3).
- `check-premises.mjs`: 16\|a `premises: none used before their card`; 2\|a 35 hits, all the stage-3 pointers (setting-part 直觉/说明 lines, `15|b|a|1`, `1|a|a|a|5`, `6|a`, `7|b|a|2`, `8|d|a`); 8\|a 1 hit, the same stage-3 pointer in the imported closure Card `2|a|a|c` (`怎样进入模型：…家庭预算约束 (2) …定价条件 (7)`), kept.
- `check-settings.mjs`: 2\|a `settings: all in the setting part, as a dependency tree` (advice only: 14 settings under the setting card; nine `uses an assumption` lines, unchanged); 8\|a and 16\|a `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"links_added":7,"cards_refused":0,"cards_changed":3,"applied":true,"ok":true,"links_repaired":0,"rounds":1,"book":"arrocannarsa2026flight"}`. All checks above, `get_model` and the formula comparison with the backup were rerun after it and stay as listed.

### Open

1. The two cross-book copy advice lines on 2\|a (`2|a|a|16` ~ gomezcram2026government/3\|i\|a\|5, `2|a|a|13|d|b` ~ gomezcram2026government/3\|i\|a\|8) stay in `check` output; judged comparisons of method.
2. The appendix B proofs of 命题 1–4 and the A.1 proof restate the main-text derivations step for step (stage-3 open item 2). Inside one model, so no import applies; both proofs stay. A later restructure may let the appendix proof cite the main-text steps.
3. 8\|a's measure imports the definition block with its closure (11 setting Cards on the 共用 shelf). 2\|a's calibration value $\nu=300$ bp keeps a plain pointer to 8\|a in chapter 7; if the owner wants the calibration to rest on the estimate, 2\|a needs a parameter card for $\nu$ first.
4. Stage-3 open items 1, 3 and 4 (result and derivation text not redrafted; the stale **分部** in 第 6 章; fourteen settings under the setting card) stay.
5. The owner decision on merging the Event Cards stays open. Current addresses (found by their 2009-03-12 content): `arrocannarsa2026flight/16|a|7|a` (2009 年 3 月瑞士国家银行开始买入外汇) and `arrocannarsa2026flight/11|b|2|a|a` (2009 年瑞士国家银行首次外汇干预), both unchanged and untouched.
6. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.

## Stage 5: illustrate

Date 2026-11-14, live box (owner's request), under `zt-model-illustrate`, done in one side session by scripts. No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-illustrate.sqlite3` (`VACUUM INTO`, 402 MB, `PRAGMA integrity_check` ok; drive D had 237 GB free). Only the formal model 2\|a was illustrated; the empirical models 8\|a and 16\|a have no figures (owner, 2026-11-11). Module: `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\arrocannarsa2026flight-2_a.mjs` (uncommitted, under `~/.claude`). Before the live write, every figure was rendered locally from the module (a copy of `verify.mjs` that reads the figures from the module, session scratchpad `local.mjs`) in light and dark, and the PNGs were inspected; faults fixed there: overlapping curve and point labels (c, d\|b, e, h\|c, i, i\|b, k, n, o\|a), a box title drawn over its own notes (b), a label cut at the left edge (i\|a), a split-bar label wider than its segment (j), a plot label over a heading (k\|b, g), a diverging path drawn flat at the frame (j\|a), and the underlined iota `ι̲` that SVG text shows like an "L" (now named in words, the caption keeps $\underline\iota$). `apply.mjs --write` then wrote 38 Cards (the Model Card, 35 setting cards, 2 定义 parts). Two later writes changed only 直觉 lines: `n` got back its mention of $a_r$ (the notation card names `n` as the card where a reader first meets $a$; without it check-settings reported `first`), and `i` and `m|b` were reworded after zt-check flagged R4 (passive 被). No display formula, equation number or text outside the chart and the **直觉** section changed (apply guard). The Model Card figure has three parts (`part()`, top-level `data-part`): 一 · 博弈与时序, 二 · 因果结构, 三 · 模型研究的变化.

### Figures

- Model Card 2\|a (**图**, before **结构**): (一) six steps of one period: the flight shock $\theta^*_t$; the H government (Taylor rule, the floor in the variants, supply $Q^s_t$ fixed or by rule, budget (13)); the H household (preferences under (2), two consumptions, labour, three assets); the Calvo firms ((10)); the F household ((14)); clearing (24)–(28) with the law of one price; then each optimizer's objective and constraint and the solution order by chapter (1–3 optimality, 4 asset pricing, 5 equilibrium, 6 linearization, 7 steady state, 8–9 baseline flight, 10–14 the variants, 15–19 appendices). (二) four rows: flight shock, supply, steady-state convenience yield $\nu$, cost $\psi$ → foreign holdings up (37), home holdings crowded out, current account (43) → convenience-yield deviation, non-tradable demand (40), appreciation → policy rate, non-tradable output and inflation, net resource transfer; the dashed feedback from the policy rate to demand. (三) twelve panels: 第 8 章 safe-haven pattern and the three absorption channels; 第 9 章 regime dependence; 第 10 章 the lower bound; 第 11 章 supply rule and the exchange-rate peg; 第 12 章 asset-leg neutrality and its break at $\psi>0$; 第 13 章 permanent flight and the threshold; 第 14 章 known duration; 第 18 章 net resource transfer. Each box opens its Card.
- 2\|a setting cards (35), at the end of **说明**: setting card (the three assets by currency and convenience, the four agent types each opening its card, the $t$ / $t+1$ timing); b (who provides convenience services, $V$ reads only $q_t$); c (the quadratic cost around $\bar f$ for two sizes of $\psi$, 示意); d (the three utility terms, the Cobb-Douglas bundle (1)); d\|a ((2) as uses against resources, minus tax and cost); d\|b (the constraint curve and two iso-spending lines, 示意; given / chosen / objective (55) / constraint); e (varieties → CES → output; the demand (8) for two elasticities, 示意); e\|a (the reset / indexed split, the survival probabilities $\alpha^k$, technology, discounting and objective (10)); f ((13) as net asset purchases against maturing returns and tax); g (F's household and government, the three choices, known and unknown at $t$); g\|a (the two utility terms, the conversion $Q^*_t\to$ real holdings); g\|b ((14) in foreign currency); g\|c (the debt recursion (17)); h (the two countries' agents, tradables and assets crossing, what equilibrium requires); h\|a (the five markets (24)–(28) as supply = holding); h\|b (the law of one price as a conversion); h\|c (a converging against a Ponzi path, 示意; the two stocks); i (flat paths, the policy-rate anchor); i\|a (the number line of the anchor $i^R=1.0212\%$ above the bound, the acute-phase window); i\|b (the four restrictions, the cost at its minimum); j (log against its tangent, 示意; the count 19 = 13 + 4 + 2); j\|a (bounded against diverging paths, 示意; the two foreign zeros); k (the AR(1) shock bars, 示意; the three fixed blocks; the fixed supply split, 示意); k\|a (the three calibrated groups, the number line $\phi_\pi=1.5>1$, the anchor window with 2009-03-12); k\|b (supply fixed: the split before and after, 示意; $\phi_\pi>1$; the tangent at $\nu$); l (baseline against variant one, row by row); l\|a (the actual rate against the rule rate, flat at the bound); m (the within-period order; baseline against variant two); m\|a (the three restrictions, the one instrument, the number line $\psi>0$); m\|b (one instrument, two targets); m\|c (the two uses of an issue, the central-bank balance sheet under each); m\|c\|a (what the comparison holds fixed, what the neutrality case adds); n (the permanent step against the decaying baseline, 示意; $\hat r$ and the steady state); o (the flight of known duration $N$ against the permanent one, 示意); o\|a (the innovations $\varepsilon_t$ in units of 36.5, drawn with $\rho_{\theta^*}=0.95$; perfect foresight and complementarity).

### 直觉

The **直觉** section was rewritten on all 35 setting cards of 2\|a and on the two 定义 parts 2\|a\|a\|6\|a (linearization coefficients) and 2\|a\|a\|7\|a (the convenience yield (52)). Every card had a digest or stage-3 section. These carried Card Links (d\|b, h\|c, i, i\|b, j, j\|a, k, k\|b, l, m, m\|b, m\|c, n, o, o\|a name chapters by `[[…]]`), later equation numbers (c (2)(7), i\|b (30)–(38), j\|a (42)(49), m (44), m\|a (43)), named results (j\|a 命题 2 and 命题 4 (iv)) and three `（补：书中未写）` marks (d\|b twice, o). The new sections name chapters in words (第 1–19 章), with no Link, no result, no later equation number and no assumption name; they use $TeX$ only for a symbol the card or an earlier setting defines ($\gamma$ in d; $a_r$ in n, where the notation card places it). `check-premises` hits on 2\|a fell from 35 to 26.

### Chart fixes outside the setting part

`verify.mjs --model` also lints the digest's own charts. Six of the 8 earlier charts failed: 1\|a\|a\|a\|4\|a (raw `C_N/C_T`; no axis labels on two plots), 7\|b\|b (raw `i^R` in three texts), 9\|c\|a (no axis labels), 11\|a\|d (raw `q^s`), 12\|b\|a (two plots without axis labels), 19\|c\|b (no axis labels). Only their chart fences changed (session scratchpad `fixcharts.mjs`, guard: text outside the fence identical): the sub- and superscripts became tspans, and the existing axis texts became labels with `data-axis` (相对价格 / 相对数量, 成本份额; 稳态便利收益（个基点）/ 柱高为 27 个基点情形的倍数; 60 季度 / 贸易品消费（%）, 家庭外国资产（%）; 衰退 / 停留期), and 19\|c\|b got one new x label, 外逃持续时间 N. No bar, curve or number changed.

### Gate

All lines below are the reruns after the Term relink.

- `verify.mjs --model arrocannarsa2026flight/2|a` (light and dark): `44 chart(s) in 257 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉` (exit 0). 0 dead targets. The PNGs, one per part, were inspected.
- `apply.mjs` rerun: `same` for all 38 Cards; `fixcharts.mjs` rerun: `same` for all 6.
- `check-settings.mjs`: `settings: all in the setting part, as a dependency tree` (advice unchanged: 14 settings under the setting card, the nine `uses an assumption` lines).
- `check-model.mjs`: `members 256 {"main":20,"side":236}`, `relations {"refers":221,"rests-on":430,"corresponds":28,"contrasts":11,"illustrates":20}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` (advice only: 第 2 章 has no `结果：` card, as at stages 3 and 4).
- `check-premises.mjs`: 26 hits (was 35), all existing **说明**/**设定** pointers (e (65)(67), e\|a (68), i\|b (31)(37)(79)(82), j (43), k\|b (39)(40)(46), l (39)(51), m (39)(40)(42), m\|a (39)(40)(42)(43)) and the stage-3 chapter pointers (`1|a|a|a|5`, `6|a`, `7|b|a|2`, `8|d|a`, `15|b|a|1`); none is in a figure or a 直觉 section.
- `zt-check --cards` on the 44 written Cards (one retry on `changing read view`): `faults: 34`, all in existing text: 31 R3 (the 概要 of 2\|a; the **设定**/**说明**/summary lines of the setting card, b, d, e\|a, g, g\|b, g\|c, h\|c, i, k, k\|a, l, m, m\|c, n, o, o\|a), 1 R4 (the summary line of l\|a) and 2 R20 advisories (k\|b `第 7 章`, m\|c\|a `命题 3`). None is in a 直觉 section or a chart. The two R4 faults on new 直觉 lines (i, m\|b) were reworded away.
- `list_broken_links` (agent): `{"broken":[]}`, before and after the relink.
- Term relink: `{"links_repaired":0,"links_added":28,"applied":true,"cards_refused":0,"book":"arrocannarsa2026flight","ok":true,"cards_changed":22,"rounds":1}`; `-PlanOnly` afterwards: `{"book":"arrocannarsa2026flight","cards_refused":0,"applied":false,"links_repaired":0,"links_added":0,"rounds":0,"ok":true,"cards_changed":0}`. All checks above were rerun after it.

### Open

1. Stage 7 (literature organize) will renumber addresses. zt 0.63+ rewrites chart `data-zt` targets with the Links on a move, but the module's `head` (`arrocannarsa2026flight/2|a`, and with it every `A()` address built from it) must be changed to the model's new address; the module has no other hard-coded address. Then run `apply.mjs --write` on the new head and `verify.mjs --model` on it, which must show 0 dead targets and `same` on a second `apply.mjs`. The digest charts fixed by `fixcharts.mjs` have no click targets; that script names its six Cards by address and is only needed again if a lint fault returns.
2. The 34 zt-check faults (31 R3, 1 R4, 2 R20 advisory) predate this pass and were not changed.
3. The 26 `check-premises` hits are existing pointers in **设定**/**说明** and chapter derivations; none is in a figure or a 直觉 section.
4. The causal figure of the Model Card has 13 boxes and reads small at 640 px; the chart sheet shows it alone as part 二. The digest chart 19\|c\|b keeps its 9 px labels; only its axis labels were added.
5. The owner decision on merging the Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a stays open; both were not touched.
6. No `card_locked`, no failed write. One `changing read view` refusal of zt-check was retried. Other sessions wrote other books on the box throughout. Nothing committed (stage brief).

## Stage 6: evidence dialogue

Date 2026-11-14, live box (owner's request), under `zt-evidence-dialogue` steps 1–5, done in one side session by scripts (`dialogue.mjs survey | apply | check`) and one DeepSeek judging conversation. No subagent ran. Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-dialogue.sqlite3` (`VACUUM INTO`, 403 MB, `PRAGMA integrity_check` ok; drive D had 236 GB free). The Terms were relinked at the end of stage 5 (`-PlanOnly` 0), so the survey ran without a first relink. Only the empirical models 8\|a and 16\|a took this pass; the formal model 2\|a is not on the dialogue's other side. No Card was written.

### Survey

The models were surveyed one at a time, 16\|a after 8\|a was applied, so the judged record dropped the mirror pair.

- `survey 8|a`: 3 estimates of ours (8\|a\|a\|3\|b 2007 年后均值 27 个基点, 3\|c 2008 年末峰值约 287 个基点, 3\|d 校准取值每年 300 个基点), 1,496 Cards of the other side, estimand Terms 便利收益 (1\|a\|b) and 代理变量 (1\|a\|ad). 1 pair in 1 message: 8\|a\|a\|3\|d (ν = 3.000% p.a.) against 16\|a\|a\|5\|i (Table 6 data, $i^R$ at the window end 0.25% p.a.), shared Term 稳态 (1\|a\|ap) only.
- `survey 16|a`: 19 estimates, 1,480 Cards, estimand Term 急性期 (1\|a\|v). Before the 8\|a apply it proposed only the mirror of the pair above; after it, 0 pairs.
- **No cross-book pair.** As in bergin2026exchange and assenza2026louder, the survey pairs on shared Term addresses, widened only by zt-term-sync Links. None of the book's 181 Term Cards has a Link to another book's Card (all read and checked), and 急性期 has no inbound Link from another book. A text search for 瑞士法郎 (22 hits) and 避风港 (32 hits) finds no Card of another book except das2026domestic/6\|a\|3 (an Idea Card, not a Finding Card or a model estimate); the search for 便利收益 stops at its 200-row limit inside this book, so it is no proof for that word. No pair was forced.

### Judging (DeepSeek)

One conversation, one message: `opencode-go/deepseek-v4.1-flash`, effort high (the box's drive model, as at stage 4), OpenCode Go account Primary (pinned), 1 request, 8 s, context 39k of 1M, no tool call. Cost: OpenCode Go subscription; the Harness ledger books 0 CNY for the source (the 2026-11-14 opencode-go day line, 3,899 calls at the end, is shared with other sessions, so there is no per-job token split). Conversation e3efdd27 deleted, workspace removed. Conversation 3b1d187b was not touched.

Answer: p1 `same: false` (the calibrated convenience yield ν against the window-end policy rate; only the Term 稳态 is shared). Checked against the two titles: right.

### Pairs judged and written

| Head | Pairs | Same estimand | Agree / disagree / not comparable | Links written |
|---|---|---|---|---|
| 8\|a | 1 | 0 | 0 / 0 / 0 | 0 |
| 16\|a | 0 | — | — | 0 |

`apply` 8\|a: `{"links":0,"written":0,"errors":0}`; the answered pair is in the judged record (`~/.zt-evidence-dialogue/judged-default.jsonl`). 16\|a had no pair, so no apply.

### Gate

All lines below are the reruns after the Term relink.

- `dialogue.mjs check`: 8\|a `{"broken":0,"proposed_again":0,"ok":true}`; 16\|a `{"broken":0,"proposed_again":0,"ok":true}`.
- No same-estimand pair, so no **对应** line to check.
- `check-settings.mjs`: 8\|a and 16\|a `settings: the data tree and the specification ladder are in place`.
- `check-model.mjs`: 8\|a `members 28 {"shared":12,"main":4,"side":12}; relations {"rests-on":24,"refers":26,"corresponds":1}; loose 0; self_sufficient true`; 16\|a `members 44 {"main":8,"side":32,"shared":4}; relations {"rests-on":26,"refers":76,"illustrates":8,"corresponds":1,"objects":1}; loose 0; self_sufficient true`.
- `list_broken_links` (agent): `{"broken":[]}`, before and after the relink.
- Term relink: `{"cards_changed":0,"ok":true,"book":"arrocannarsa2026flight","links_added":0,"rounds":0,"links_repaired":0,"applied":false,"cards_refused":0}`.

### Open

1. No cross-book pair: the book's Terms (便利收益, 代理变量, 急性期, …) link to no Term Card of another book. After a zt-term-sync pass on the economics shelf, survey both heads again; the judged record keeps the pair judged here from being paid for twice. The hand-written cross-book Links of 16\|a (`对照 [[becker2025division/5|b|b|8]]`) and the stage-4 comparisons on 2\|a stay as written.
2. Within the book, the measured convenience yield of 8\|a and the Table 6 rows of 16\|a share no estimate-level Term, so the survey proposes no pair between, for example, the 287 bp peak (8\|a\|a\|3\|c) and Table 6's data rows. The relation of 16\|a to 8\|a is the theory's (both rest on 2\|a); none was forced here.
3. Script observation, not changed: the survey's Term widening (`equiv` in `dialogue.mjs`) takes every non-Term Link of an estimand Term Card, so the book root `arrocannarsa2026flight` (from each Term Card's source line) appears in `estimand_terms`. It forms no pair (pairs match Term-link targets only, and no Card Term-links the book root), so it was left; a later fix can keep only Term Cards of other books.
4. The owner decision on merging the Event Cards 16\|a\|7\|a and 11\|b\|2\|a\|a stays open; both were not touched.
5. No `card_locked`, no `read_view_changed` and no failed read or write occurred. Other sessions wrote other books on the box throughout. Nothing committed (stage brief).

## Stage 7: literature organize

Date 2026-11-14, live box (owner's request), under `zt-literature-organize`, done in one side session by scripts (`outline.mjs`, one structure batch through the skill's `zt.mjs`, `leads.mjs`, `check.mjs`, and small `edit_card` scripts for link fixes, titles, source lines, the root index and the structure card; session scratchpad `org/`). No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-organize.sqlite3` (`VACUUM INTO`, 404 MB, `PRAGMA integrity_check` ok; drive D had 236 GB free). The structure batch was previewed first (complete true, refusals none, `links_that_will_break` empty, 891 Cards renamed) and committed under its plan hash. The tree went from 892 to 894 Cards (two new top Cards: 前置页 and the §5 summary). No Card was deleted, and no formula, equation number or estimate changed. Only Cards of arrocannarsa2026flight were written. No `card_locked`, no `read_view_changed` and no failed batch occurred; the tree was unlocked throughout. Other sessions wrote other books on the box.

### Source divisions

From `D:\zt-digest\arrocannarsa2026flight\book.zh.md`: the title page (title, author with unit, first version 2025-11, this version 2026-10-27, award footnote) and 摘要 (with JEL codes and keywords); 1 引言; 2 模型 (2.1 一般框架, 2.2 H 国, 2.3 F 国, 2.4 资产市场关系); 3 均衡 (3.1 对数线性化一般均衡); 4 校准 (4.1 稳态识别, 4.2 便利收益, 4.3 资产组合头寸, 4.4 其余参数, 4.5 冲击); 5 结果 (5.1–5.7); 6 结论; A 推导 (A.1, A.2, A.3); B 证明; C 结果 (C.1–C.8). The Markdown has no reference list and no third-level numbered division. The top line takes the numbered sections and the three appendices, with the front matter first.

The digest had filed the paper as 28 group Cards, mostly one per subsection (old 3 = 2.3, old 4 = 2.4, old 6 = 3.1, old 8–10 = 4.2–4.4, old 12–17 = 5.2–5.7, old 20 = A.3, old 23–28 = C.3–C.8), with four crossers (old 2 = §2–2.2, old 7 = §4 + 4.1, old 11 = 4.5 + 5.1, old 19 = A.1 + A.2, old 22 = §C + C.1 + C.2), and the Term list, the Cited list and the structure Card under old 1 (引言).

### Final tree (top line and divisions)

```
1   前置页 (new Card)                    a 结构卡 (old 1|e, rewritten)
2   1 引言 (old 1, edited)               c §1 chain (old 1|c–1|c|12, events c|1|b, c|1|c) · d finding (old 1|d)
3   2 模型 (old 2, rewritten)            a 2.1 (old 2|b–2|b|2) · b 2.2 (old 2|b|3–12) · c 2.3 (old 3) · d 2.4 (old 4)
                                         e formal model (old 2|a)
4   3 均衡 (old 5, rewritten)            b §3 notes (old 5|b–5|b|9) · c 3.1 (old 6)
5   4 校准 (old 7, rewritten)            a §4 preamble (old 7|d–7|d|2)
                                         b 4.1 (old 7|d|3–6; findings b|a old 7|b, b|3|a old 7|c)
                                         c 4.2 (old 8; c|a empirical model old 8|a) · d 4.3 (old 9) · e 4.4 (old 10)
                                         f 4.5 (old 11|b–11|b|1; finding f|1|a old 11|c)
6   5 结果 (new summary)                 a 5.1 (old 11, rewritten; chain a|d old 11|b|2–9, event a|d|a|a)
                                         b 5.2 (old 12) · c 5.3 (old 13) · d 5.4 (old 14) · e 5.5 (old 15)
                                         f 5.6 (old 16; f|a empirical model old 16|a, event f|a|7|a) · g 5.7 (old 17)
7   6 结论 (old 18, retitled)
8   A 推导 (old 19, rewritten)           c A.1 (old 19|c chain) · d A.2 (old 19|d chain) · e A.3 (old 20)
9   B 证明 (old 21)
10  C 结果 (old 22, rewritten)           a §C preamble (old 22|a) · b C.1 (old 22|a|1–3; findings old 22|b–d)
                                         c C.2 (old 22|a|4–8; findings old 22|e–i)
                                         d C.3 (old 23) · e C.4 (old 24) · f C.5 (old 25) · g C.6 (old 26)
                                         h C.7 (old 27) · i C.8 (old 28)
11  术语表 (Term list, from 1|a)
12  参考文献 (Cited list, from 1|b)
```

One structure batch (51 items: 2 creates, 45 moves, 6 reorders, 891 renamed), then `leads.mjs` (eight rewrites, no demote), then `edit_card` scripts.

### Crossers and how each was settled

- Old 1 held §1 (and named 摘要 in its index), the two lists and the structure Card. The lists went to the tail (`11`, `12`). A new 前置页 Card took the first place on the top line, with the structure Card under it (`1|a`). Old 1 is the §1 Card (`2`): retitled `1 引言：…`, its `§摘要` index line dropped, its position Links for §4 and §5 redirected to the new section summaries. 2\|c keeps its source line `§摘要、§1`: its text is the opening of §1.
- Old 2 (§2–2.2) already introduced the model framework: rewritten as the §2 summary (`3`). Its chain was cut by source line into 2.1 (`3|a`, three Cards) and 2.2 (`3|b`, ten Cards); old 3 and old 4 moved under it as 2.3 and 2.4; the Model Card 2\|a became its last Side branch `3|e`.
- Old 5 spoke for §3: rewritten as the §3 summary (`4`), with its §3 chain first and old 6 (3.1) after it.
- Old 7 (§4 + 4.1) introduced the calibration: rewritten as the §4 summary (`5`). Its chain was cut into the §4 preamble (`5|a`, source line `§4`) and 4.1 (`5|b`, source line `§4.1`); the two 4.1 findings went under the notes that state them.
- Old 11 (4.5 + 5.1): the 4.5 notes (old 11\|b, 11\|b\|1, two Cards and one finding) went under §4 as 4.5 (`5|f`); old 11, with eight 5.1 notes, is the 5.1 head (`6|a`), rewritten to speak for 5.1 only. §5 had no Card that spoke for it, so a new summary (`6`) heads 5.1–5.7.
- Old 19 (A.1 + A.2) became the A summary (`8`) with old 20 (A.3) under it. Old 22 (§C + C.1 + C.2) became the C summary (`10`): its chain was cut into the §C preamble, C.1 and C.2, its findings went under C.1 and C.2, and old 23–28 moved under it.
- Within 5.3, 5.5, C.5 and C.8 the reading notes now come before the Finding Cards (the digest had filed the findings first).
- Links that a move made false were redirected (text unchanged otherwise): `3|c` (2.1/2.2 → `3|a`, `3|b`), `5|c`, `5|d`, `5|e` (4.1 → `5|b`; `5|e` also 4.5 → `5|f`), `3|e|a|16` (model part: 稳态锚 → `5|b`), `8|e` (A.2 → `8|d`, 3.1 → `4|c`, §4 → `5`), `8|e|b` (3.1 → `4|c`, a digest error that named the calibration group), `10|d`, `10|e`, `10|e|a`, `10|h` (C.1 → `10|b`, and C.2 → `10|c` in `10|d`), `5|f|1|a` (source line from the old 11 group to the book root, §4.5).

Calls that could go the other way: the front matter as its own top Card written from the source (title page and 摘要), since no digest Card held the abstract; 2\|c (`§摘要、§1`) left in §1; old 2, old 7, old 19 and old 22 rewritten as summaries rather than demoted (each already introduced its whole division); 2.1 and 2.2 headed by their first reading notes (`3|a`, `3|b`), which keep their titles; 2\|a filed as the last Side branch of the §2 summary (it spans §2–§5 and A–C; its head's source line starts at §2, as gomezcram filed 2\|a); 8\|a under 4.2 and 16\|a under 5.6 by their source lines; the findings moved behind the notes in four divisions.

### Summary leads and other text

- Rewrites by `leads.mjs` (Term names turned into Term uses): `1` (前置页), `3` (2 模型), `4` (3 均衡), `5` (4 校准), `6` (5 结果), `6|a` (5.1), `8` (A 推导), `10` (C 结果). Each has the source's name and main claim as title, a paragraph that walks the subdivisions, `小节索引：`, `在全书中的位置：` and the source line. The old `对应` Links of old 5, old 7 and old 19 were kept. Formulas and equation numbers the old text carried are copied unchanged.
- Division heads retitled with their section number: `3|c` (2.3), `3|d` (2.4), `5|c`–`5|e` (4.2–4.4, 4.4 drops 校准), `6|b`–`6|f` (5.2–5.6), `7` (6), `10|d`, `10|e`, `10|g`, `10|h` (C.3, C.4, C.6, C.7); `2` retitled `1 引言：…`. Source lines added to `3|c`, `5|d`, `5|e`, `6|c`, `6|f`, `10|d`, `10|e`, `10|g`, `10|h` (zt-check I3).
- The root index lists the twelve top Cards. The structure Card `1|a` adds the 前置页 and §5 lines and lists the three models with their levels (its old model list named the chapters of 2\|a as models, stale since the fold); its events list was kept.
- `zt-check --cards` on the root and the 30 written Cards: first run 16 faults (R2 ×2 in the root index; R4 and R2 ×2 in `1|a`; I3 ×10 on group heads without a source line; F2 and L1 on `5|f|1|a`), fixed once; second run `faults: 0`, exit 0.

### Chart targets

Backup: `D:\zt\backups\zt-before-arrocannarsa2026flight-chart-targets.sqlite3` (integrity ok). In `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\arrocannarsa2026flight-2_a.mjs` the `head` is now `arrocannarsa2026flight/3|e`, and the header comment names the new head and the old one; the module has no other hard-coded address (every target is built from `head`), and the file name stays. `apply.mjs --write`: `same` for all 38 Cards, `nothing to write`: zt had already rewritten every `data-zt` target with the moves. Gate: `verify.mjs --model arrocannarsa2026flight/3|e` (light and dark): `44 chart(s) in 257 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉` (exit 0); 0 dead targets (`dead-target` is a lint rule).

### Gate

- `check.mjs arrocannarsa2026flight` (against the last outline): top line 1 前置页 (2), 2 1 引言 (19), 3 2 模型 (284), 4 3 均衡 (22), 5 4 校准 (58), 6 5 结果 (142), 7 6 结论 (8), 8 A 推导 (16), 9 B 证明 (13), 10 C 结果 (98), 11 Term list (181), 12 Cited list (50); `no fault`. Against the start snapshot the only fault is `892 Cards before, 894 now` (the two new Cards); no whole unit changed size (2\|a 257, 8\|a 21, 16\|a 51, Term list 181, Cited list 50).
- Source-line walk (322 Cards with a source line, whole units excluded): every Card under its own top division, and every second-level branch holds only its own section's label. No crosser left.
- `list_broken_links` (agent): `{"broken":[]}`.
- `check-model.mjs`: `3|e` `members 256 {"main":20,"side":236}`, `relations {"refers":221,"rests-on":430,"corresponds":28,"contrasts":11,"illustrates":20}; loose 0; self_sufficient true`; `5|c|a` `members 28 {"shared":12,"main":4,"side":12}`, `relations {"rests-on":24,"refers":26,"corresponds":1}; loose 0; self_sufficient true`; `6|f|a` `members 44 {"main":8,"side":32,"shared":4}`, `relations {"rests-on":26,"refers":76,"illustrates":8,"corresponds":1,"objects":1}; loose 0; self_sufficient true` (all as at stages 5–6).
- `check-network.mjs`: `network: clean` on all three.
- `check-settings.mjs`: `3|e` `settings: all in the setting part, as a dependency tree`; `5|c|a`, `6|f|a` `settings: the data tree and the specification ladder are in place`.
- `imports.mjs check`: exit 0 on all three.
- Term relink: `{"links_added":0,"links_repaired":0,"rounds":0,"ok":true,"applied":false,"cards_refused":0,"book":"arrocannarsa2026flight","cards_changed":0}` (`leads.mjs` had already turned the Term names of the new text into Term uses); `-PlanOnly`: `{"cards_refused":0,"cards_changed":0,"links_added":0,"rounds":0,"book":"arrocannarsa2026flight","applied":false,"ok":true,"links_repaired":0}`. No relink bug. The relink changed no Card, so the checks above stand after it.

## Result

Final top line: 前置页 · 1 引言 · 2 模型 · 3 均衡 · 4 校准 · 5 结果 · 6 结论 · A 推导 · B 证明 · C 结果 · 术语表 · 参考文献.

Models (final addresses):

- `arrocannarsa2026flight/3|e`, formal (level 2): the two-country flight-to-safety model (was 2\|a), 256 parts, setting card `3|e|a`, illustrated (44 charts); filed as the last Side branch of the §2 summary.
- `arrocannarsa2026flight/5|c|a`, empirical (level 3): the Swiss franc convenience-yield measurement (was 8\|a), 28 members (12 imported); filed under the 4.2 head.
- `arrocannarsa2026flight/6|f|a`, empirical (level 3): the Table 6 acute-phase comparison (was 16\|a), 44 members (4 imported); filed under the 5.6 head.

Event Cards awaiting the owner's merge decision (both record the 2009-03-12 SNB FX intervention; not merged, not edited): `arrocannarsa2026flight/6|f|a|7|a` (was 16\|a\|7\|a, inside the 5.6 model) and `arrocannarsa2026flight/6|a|d|a|a` (was 11\|b\|2\|a\|a, under the 5.1 footnote card). Owner decision 2026-11-14: leave both; they are not merged.

All seven stages passed their gates. Open items across the sequence:

1. Closed 2026-11-14: the owner decided to leave both Event Cards above as they are.
2. The 34 zt-check faults in existing text of 2\|a (31 R3, 1 R4, 2 R20 advisory) and the 26 `check-premises` pointers predate stage 5 and were not changed (Stage 5).
3. No cross-book evidence pair: survey 8\|a and 16\|a (now `5|c|a`, `6|f|a`) again after a zt-term-sync pass on the economics shelf; the `dialogue.mjs` Term widening also takes the book root (Stage 6).
4. The causal figure of the Model Card reads small at 640 px; the digest chart 19\|c\|b (now inside 3\|e) keeps 9 px labels (Stage 5).
5. The illustrate module keeps its file name `arrocannarsa2026flight-2_a.mjs`; only `head` and the header comment changed (Stage 7).
6. The front-matter Card `1` is written from the source's title page and 摘要, not from digest Cards (Stage 7).
7. This pass doc is not committed (the stage brief said no commit).
