# ozhan2026monetary: post-digest model passes

Ozhan, Sander, Wende and Yang, *Monetary Policy Under Network-Level Bottlenecks* (IMF WP/26/244), Work 29957. The digest is complete in its ledger (`run_complete:true`); its run report lists one gap, a figure the source gives no label. The Bridge was rebuilt fresh on 2026-11-14. All passes ran on the live box (`D:\zt`), 2026-11-14.

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-ozhan2026monetary-fold.sqlite3` (integrity_check ok).

The survey found 22 Model Cards (5 concept, 15 formal, 2 empirical). Several rows needed judgement (10\|a, the Appendix A cards, 19\|a, 20\|a). The proposal was shown to the owner, who approved it on 2026-11-14 ("Yes, fold as proposed").

| Model Card | Level | Model | Surviving head | Reason |
|---|---|---|---|---|
| 5\|a | formal | stylized model (§2) | 5\|a | households and upstream production with a task-based capacity cap |
| 6\|a | formal | stylized model | 5\|a | §2.2.2: downstream production of the same two-sector economy |
| 7\|a | formal | stylized model | 5\|a | §2.3: market clearing and the feedback rule that close the same model |
| 8\|a | formal | quantitative model (§3) | 8\|a | two regions, 11 sectors, two household types |
| 3\|a | concept | quantitative model | 8\|a | the §1 verbal version of the quantitative model; first chapter |
| 9\|a | formal | quantitative model | 8\|a | §3.2.1: the firms' CES production and labour cap |
| 9\|b | formal | quantitative model | 8\|a | §3.2.2: Calvo pricing and dominant-currency pricing |
| 10\|a | formal | quantitative model | 8\|a | §3.3–3.5: Taylor rule, government and market clearing of the same model; its rest on the stylized feedback rule is a borrowed result, kept as an **引入** line |
| 11\|a | formal | quantitative model | 8\|a | §3.6: extensions of the same setting (sticky wages, imperfect labour mobility, fixed capital) |
| 17\|a | concept | quantitative model | 8\|a | §5.2: feature-by-feature experiment on the same model (Hulten benchmark) |
| 18\|a | concept | quantitative model | 8\|a | §5.3: sectoral labour-cap experiment on the same model (Phillips curve) |
| 22\|b, 23\|a, 24\|a, 25\|a, 26\|a, 27\|a | formal | quantitative model | 8\|a | Appendix A: the full OLG form of the same model's households, aggregates, demand system, production, pricing and closure |
| 13\|a | empirical | fit to 2020–24 data (§4, §6.2) | 13\|a | its own data (48 observables, two regions, 11 sectors, 2020Q1–2023Q4) and its own shock extraction; it takes the quantitative model as a whole |
| 14\|a | empirical | fit to data | 13\|a | §4.2: the quasi-nonlinear shock extraction on the same data |
| 15\|a | concept | fit to data | 13\|a | §4.3: the counterfactual method that keeps the estimated constraint states |
| 20\|a | concept | fit to data | 13\|a | §6.2: the decomposition of the fitted supply-constraint contribution |
| 19\|a | formal | DC-index weighting experiment (§6.1) | 19\|a | own question (how to weight sectoral inflation) and own multi-sector setting |

The book holds **4 models**: `5|a` (formal), `8|a` (formal), `13|a` (empirical) and `19|a` (formal).

### Fold (executed 2026-11-14, live box)

- 5\|a: `5|a|a|9` 第 2 章：下游生产与迂回投入 (was 6\|a), `5|a|a|10` 第 3 章：市场出清与利率规则 (was 7\|a).
- 8\|a: `8|a|a|1` 第 1 章：文字版本：网络与劳动上限 (was 3\|a); `8|a|a|17` 企业生产 (9\|a); `|18` 企业定价 (9\|b); `|19` 政策、政府与市场出清 (10\|a); `|20` 扩展 (11\|a); `|21` Hulten 基准与模型特征 (17\|a); `|22` 劳动约束与菲利普斯曲线 (18\|a); `|23`–`|28` the six Appendix A chapters (22\|b, 23\|a–27\|a).
- 13\|a: `13|a|a|4` 准非线性冲击提取 (14\|a), `|5` 准非线性反事实模拟 (15\|a), `|6` 供给约束的贡献分解 (20\|a).
- Each folded card's setting or data parts are filed under it; its Direct idea notes moved to its section as Side branches; the idea Side cards of 15\|a stay in section 15.
- The dropped **引入** lines of 6\|a, 7\|a, 9\|a, 9\|b, 23\|a and part of 10\|a named parts of their own model after the fold. 10\|a's line to the stylized model's feedback rule (`5|a|a|10|a|1`) names another model; it was appended to the **引入** section of 8\|a, which removed the closure fault.

### Gate

- `model-survey.mjs`: exactly 4 Model Cards: 5\|a (30 parts), 8\|a (148), 13\|a (45), 19\|a (9). 8\|a builds on 5\|a by the imported rule; 5\|a has one `false share` to a part of 8\|a, left to the import stage.
- `check-model.mjs`: every former head is a part; 19\|a `self_sufficient` true. The remaining faults are the unanchored folded chapters (5\|a: 2; 8\|a: 13; 13\|a: 3) and loose parts (1, 2, 5), which the settings and restructure passes fix.
- `list_broken_links`: empty. `term-relink.ps1`: `{"links_added":3,"cards_changed":2,"ok":true}`.

### Open

- 8\|a's Appendix A chapters restate much of §3 (the appendix text says so: "书中此处与 7.1 逐字相同"); the settings and restructure passes merge restatements.
- 5\|a's false share to 8\|a and the stylized-rule import become proper **引入** lines in stage 4.

## Stage 2: settings (strict)

Date 2026-11-14, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-ozhan2026monetary-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The moves, the deletes and the creation of the three notation cards ran as four `apply_batch` tree batches, one per model (preview, then commit under the plan hash; `links_that_will_break` empty in all four). Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the equation numbers (`\tag{…}` and `\quad (N)`) of the old and new bodies, and every written card was read back and compared with the intended text (0 differences). No display formula and no equation number changed in a card that stays. The display formulas of the deleted appendix restatements were carried byte for byte into the cards they were merged into (named below), and two display formulas were added as 补写 equations in 13\|a. No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 721 Cards to 711: three notation cards were created (5\|a, 8\|a, 19\|a), nine restating setting cards of 8\|a and four folded data-tree cards of 13\|a were deleted.

### 5\|a (formal)

The two assumptions on the main line (former 5\|a\|a\|1, \|3) and the setting cards of the two folded chapters moved into the setting part, so the derivations of §2.2.1 now stand on the main line as 5\|a\|a\|1–6 and the folded chapters renumbered: 第 2 章 is now `5|a|a|7`, 第 3 章 `5|a|a|8`. A chapter setting moved by `move_span` alone, so the derivation chain that stood on its Direct line rose into the chapter's Side slot `a`.

```
5|a|a   设定：家庭、两部门企业与两项任务的技术 (1)(2)(4)(5)          原始设定
├ a  符号表与读法 (new; blocks: 家庭与上游技术, 下游最终品企业, 市场出清与利率规则, 推导中的辅助符号, 术语)
├ b  假设：完全预见与名义工资刚性 (from 5|a|a|1)                     ← 5|a|a
├ c  假设：劳动可流动、完全竞争与外生上限 (from 5|a|a|3)             ← 5|a|a, b
├ d  设定：最终品企业用商品、服务与最终品自身作投入 (8) (from 9|a)    ← 5|a|a, b
│    d|a 假设：完全竞争、工资外生与中间投入按最终品价格计价 (from 9|a|1)  ← d, c
└ e  设定：最终品市场出清 (from 10|a)                                 ← 5|a|a, d
     e|a 假设：中央银行按带惯性的反馈规则设定利率 (from 10|a|1)       ← e
```

- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**. The three symbol tables and the term tables went into the notation card (32 rows, 6 terms). The feedback rule `e|a` is the card 8\|a imports; its **引入** line followed the move.
- Statements moved: the setting card took the discrete time line; `d|a` dropped the restated wage exogeneity and labour mobility of `c` and keeps one line (both upstream sectors face the same $W$); `e` dropped the restated timing and perfect foresight; the policy rule's pointer moved to **说明** of `e`.
- Definitions added at first use: $C_t$, $u(C_t)$, $P_t$, $B_t$, $W_t$, $L_t$, $D_t$, $i_t$, $L^j_{A,t}$, $L^j_{B,t}$, $\alpha$ in the setting card; $Y^g_{i,t}$, $Y^s_{i,t}$, $X_{i,t}$ in `d`; $P^f_t$, $MC_t$ in `d|a`; $\pi_t$ and $x_t$ in `e`, $\phi_\pi$ and $\phi_x$ in `e|a` (the source defines none of the four; each is marked 补).

### 8\|a (formal)

The three settings on the main line (former 8\|a\|a\|2–4) and the setting cards of every folded chapter moved into the setting part. The household derivations of §3.1 now stand on the main line as 8\|a\|a\|2–13 and the chapters renumbered: 第 3 章 is now 8\|a\|a\|14, … 第 8 章 \|19, the six Appendix A chapters \|20–\|25.

```
8|a|a   设定：两区域多部门与两类家庭，单期效用与预算约束                 原始设定
├ a  符号表与读法 (new; blocks: 区域、部门与家庭, 期内支出, 生产, 定价, 政策, OLG, 扩展, 术语)
├ b  设定：概述（§1） (from 1|a)                                         原始设定
├ c  设定：期内支出的两层嵌套 CES (from 2; 25|a merged)                   ← 8|a|a
│    c|a 设定：需求移动因子 (from 3)                                      ← c
├ d  假设：弹性工资、全球交易的无风险债券与外生消费习惯 (from 4)          ← 8|a|a
├ e  设定：企业 CES 生产 (19) (from 17|a)                                ← 8|a|a, d
│    e|a 设定：中间品两层 CES (from 17|a|1; 25|a|7 merged)                ← e, c
│    e|b 设定：时变外生的劳动上限 (from 17|a|2)                           ← e
│        e|b|a 假设：上限在正常时期很少起约束作用 (from 17|a|3)          ← e|b, d
├ f  设定：Calvo 定价与主导货币 (from 18|a; 27|a merged)                  ← 8|a|a, e
│    f|a 假设：国内销售的线性化菲利普斯曲线 (from 18|a|1)                 ← f, 8|a|a, e
│        f|a|a 假设：通胀预期偏离理性预期 (from 18|a|4; 27|a|10 merged)   ← f|a
├ g  设定：政策、政府与市场 (from 19|a; 28|a merged)                      ← 8|a|a, f
│    g|a 假设：泰勒规则 (from 19|a|1; 28|a|8 merged)                      ← g, 5|a|a|e|a (imported)
│    g|b 假设：补贴消除稳态加成 (from 19|a|2)                             ← g
│    g|c 假设：世界净国外资产为零 (from 28|a|5)                           ← g
│        g|c|a 假设：瓦尔拉斯定律与计价单位 (from 28|a|6)                 ← g|c
├ h  设定：实验：部门递增的劳动上限 (from 22|a)                           实验 ← e|b, g|a
├ i  设定：变体一：OLG 家庭 (from 23|a; 24|a merged)                      变体一 ← 8|a|a
│    i|a 设定：终生效用与单期效用 (from 23|a|1)                           ← i
│        i|a|a 设定：OLG 预算约束 (from 23|a|2)                           ← i|a
│            i|a|a|a 假设：横截性条件 (from 23|a|4)                       ← i|a|a
│    i|b 假设：各队列消费增长率相同 (from 23|a|9)                         ← i
│    i|c 设定：HtM 家庭消费 (from 24|a|7)                                 ← i
└ j  设定：变体二：扩展（§3.6） (from 20|a)                               变体二 ← 8|a|a, e
     j|a 假设：工资粘性 (from 20|a|1)                                     ← j
         j|a|a 设定：工资菲利普斯曲线（总量，附录 7.1.1） (from 24|a|2)    ← j|a, i, i|c
             j|a|a|a 假设：工资通胀预期 (from 24|a|3)                     ← j|a|a
     j|b 假设：CET 劳动 (23) (from 20|a|2; 28|a|9 merged)                 ← j
         j|b|a 假设：劳动市场出清 (from 20|a|6)                           ← j|b, j
     j|c 假设：固定资本 (from 20|a|7; 26|a merged)                        ← j, e
```

(Addresses in parentheses are the former 8\|a\|a\|… addresses.)

- Merged (deleted `into` the card each restates, Links re-pointed; each was first moved out as a leaf so its derivation chain stayed in its chapter): 25\|a (A.1.2 household two-layer CES) into `c`; 25\|a\|7 (A.1.2 firm intermediate demand) into `e|a`; 27\|a (A.3 Calvo and DCP) into `f`; 27\|a\|10 (A.3 expectation rules) into `f|a|a`; 28\|a (A.4 regions and central bank) into `g`; 28\|a\|8 (A.5 Taylor rule) into `g|a`; 28\|a\|9 (A.6 CET) into `j|b`; 26\|a (A.3 three-input production with capital) into `j|c`; 24\|a (A.1.1 regions, household types, transfer $G$) into `i`. Their new statements went into the kept card as one item each, headed `附录 … 的写法`, with the appendix notation ($\gamma^c$, $\gamma^x$, $IC$, $PY$, $PX$, $PZ$, $PK$, $PIC$, $C^d_{o,i,t}$, $\omega^{d,j}_{o,i}$).
- Display formulas carried byte for byte with the merges: the A.1.2 补写 aggregator (into `c`), the two A.3 expectation rules (into `f|a|a`), the A.5 Taylor rule in its bracketed form (into `g|a`, beside the §3.3 form), the A.6 CET aggregator 补写 (into `j|b`, with a note that its weight exponent is $-1/\gamma_l$ against $1/\gamma_l$ in (23)).
- The OLG block and the three extensions are variants on their own branches after the base blocks; the §5.3 sectoral-cap experiment is a top-level setting after the policy block. 24\|a\|2–3 (the aggregate wage Phillips curve and its expectation rule of A.1.1) hang under the sticky-wage assumption `j|a`, since the base model has flexible wages.
- Premises reversed: settings that rested on chapter cards now rest on settings (`f|a|a` on `f|a`, `j|a|a` on `j|a`, `i`, `i|c`, `j|b|a` on `j|b`); the former chapter premises (the derivation 15\|a\|3, the definition 21\|a, the result 17\|a\|5) are pointers in **说明**. The **引入** line of the head (the stylized feedback rule, now `5|a|a|e|a`) was left as it is; `g|a` names it in **起点**.
- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**. The fourteen symbol tables and the term tables went into the notation card (67 rows, 25 terms). One row per letter: a letter with several uses in §3 and Appendix A names all of them ($c$, $i$, $\mu$, $\kappa$, $P$, $b$, $W$, $\gamma$, $C$, $\pi$, $\rho$, $X$, $N$, $\bar L$, $\omega$); multi-letter names have their own rows ($NFA$, $MRS$, $MC$, $IC$, $PY$, $PX$, $PZ$, $PK$, $PIC$, $sp$). The rows also cover the auxiliary $X$ and $\rho$ that two Appendix derivations introduce with 记.
- Definitions added at first use in the setting card: $t$, $z$ (with $\mathrm{R}$, $\mathrm{HtM}$), $\theta_c$, $\beta_c$, $c_{c,z,t}$, $h_{c,z,t}$, $\bar h_{c,t}$, $\bar c_{c,z,t-1}$, $P^{CPI}_{c,t}$, $b^{\$c}$, $b^{\$c^*}$, $W_{c,t}$, $\Pi_{c,z,t}$, $T_{c,z,t}$; region names written as $\mathrm{US}$, $\mathrm{ROW}$ in inline formulas.
- `g|b` dropped its restatement of the government transfer (`repeat`) and points to `g`. The 变体 line of `j` names variant one without a Link, because the Link made a `relaxes` relation to `i`.

### 13\|a (empirical)

The folded chapter 4 (`13|a|a|4`, §4.2) carried a second data card, 变量表, design, four identifying assumptions and a two-step specification ladder. It was dissolved into the model's data tree, design and ladder in one batch of 12 items.

- Deleted `into` the card each restates (Links re-pointed): the §4.2 data card into the data card `13|a|a` (its list of observables became item 4); the §4.2 变量表 into `13|a|a|a`; the §4.2 design into the design `13|a|a|1` (its variation source, the exactly identified comparison and its threat were merged in); the §4.2 规格：基准 (state space and observation equation) into `13|a|a|2`.
- Moved: the §4.2 assumptions 1–4 to the design as 假设 4–7 (`13|a|a|1|d`–`g`, titles and facets renumbered, each with **起点**); the sign-restriction variant to the ladder as `13|a|a|2|a` under the baseline. Chapter 4 now opens on its hypothesis card (`13|a|a|4|a`) with its estimates (`4|a|1`) and tests (`4|a|2`); the two tests cite 假设 4 and 假设 5 by Link.
- The five data-tree cards `13|a|a|b`–`f` now have a one-line summary, **起点** and **内容**. The 变量表 lost its unused $\rho$ row, took the eight rows of the deleted §4.2 table, and its 首次出现 cells now link cards.
- Equations (owner rule of 2026-11-12): the baseline `13|a|a|2` writes the measurement equation $y^k_{c,t}=g^k(\Theta,x_t)$ and the state-space form $Y_t=Hs_t$, $s_t=Gs_{t-1}+\Psi\varepsilon_t$ (the last two carried from the deleted §4.2 baseline); its internal file reference became §4.1. The variant `13|a|a|2|a` now writes its whole system as a 补写 equation, $Y_t=Hs_t,\ s_t=Gs_{t-1}+\Psi\varepsilon_t,\ \varepsilon_t\supseteq\{\lambda^L_{i,t},\varepsilon^A_{i,t}\}$, above the kept sign restriction $\lambda^L_{i,t}\ge0,\ \lambda^L_{i,t}>0\Rightarrow\varepsilon^A_{i,t}=0$, and names 假设 4–6 in **相对基准的改动**.

### 19\|a (formal)

```
19|a|a  设定：投入产出网络、Calvo 粘性价格与盯住 DC 指数的规则   原始设定
├ a  符号表与读法 (new; 11 rows, 3 terms)
├ b  假设：产出约束 (from 19|a|a|1, main line)                    ← 19|a|a
└ c  假设：需求冲击与时点 (from 19|a|a|2, main line)              ← b, 19|a|a
```

The two assumptions left the main line, so the mechanism, the results and the derivation now stand as 19\|a\|a\|1–5. The setting card is split into seven items, defines the indices and, at first use, the slope $\kappa_{c,i}$ and the DC weight $\omega^D_{c,i}=\mathrm{Domar}_{c,i}/\kappa_{c,i}$ (inline; the derivation `19|a|a|3` uses both); `c` dropped its restatement of the central bank's information problem.

The **结构** sections of the four Model Cards were rebuilt from the build order (setting part or data tree, design and ladder first, then the main line).

### Gate

- `check-settings.mjs ozhan2026monetary/5|a`: `settings: all in the setting part, as a dependency tree`.
- `check-settings.mjs ozhan2026monetary/8|a`: `settings: all in the setting part, as a dependency tree` (advice only: the setting card has 9 settings filed under it; `b` and `h` state one thing each).
- `check-settings.mjs ozhan2026monetary/13|a`: `settings: the data tree and the specification ladder are in place`.
- `check-settings.mjs ozhan2026monetary/19|a`: `settings: all in the setting part, as a dependency tree`.
- `check-model.mjs 5|a`: 31 members (9 main, 21 side, 1 shared), loose 1; `unanchored` on the two folded chapters (`5|a|a|7`, `|8`), as after stage 1. `8|a`: 141 members (26 main, 114 side, 1 shared), loose 2, 13 `unanchored` chapter cards, no `relaxes` relation. `13|a`: 41 members, loose 5, `unanchored` on chapters `13|a|a|4`–`6`. `19|a`: 10 members, loose 0, `self_sufficient` true. The unanchored chapters are left to the restructure.
- `list_broken_links` (agent): empty, before and after the relink.
- Term relink: `{"links_repaired":0,"book":"ozhan2026monetary","links_added":72,"cards_changed":31,"applied":true,"ok":true,"rounds":1,"cards_refused":0}`. One later edit (the 变体 line of `8|a|a|j`) was followed by a second relink: `{"book":"ozhan2026monetary","links_added":1,"cards_refused":0,"ok":true,"rounds":1,"applied":true,"cards_changed":1,"links_repaired":0}`. The four settings checks and `list_broken_links` were rerun after it and stay clean.

### Open

1. All four heads: the folded chapters are unanchored (5\|a: 2; 8\|a: 13; 13\|a: 3); in 8\|a the household derivations of §3.1 (8\|a\|a\|2–13) stand on the main line before chapter 3. The restructure builds the spine.
2. 8\|a: the Appendix A chapters (\|20–\|25) still hold derivations that restate §3 derivations in appendix notation (for example the CES price indices of chapter 22 against 8\|a\|a\|12, the production block of chapter 23 against chapter 3). The settings pass merged only the setting cards; the restructure decides whether the appendix derivations merge into the §3 chapters.
3. 8\|a: two notations live side by side (§3: $c$, $\gamma_c$, $\gamma_m$, $MC$, $P^{CPI}$; Appendix A: $C$, $\gamma^c$, $\gamma^x$, $PZ$, $P^C$). The notation card names both in one row each; no formula was rewritten.
4. 8\|a: the A.6 CET aggregator (a 补写 of the digest, now in `j|b`) has the weight exponent $-1/\gamma_l$, (23) has $1/\gamma_l$; the source text does not settle which is right.
5. 8\|a: the setting card carries nine settings (advice `wide`); its blocks are named in **说明**. The check reads $C$ and $M$ first in the setting card (in the superscript $CPI$ and the label $\mathrm{HtM}$) and $N$ first in `c` (the upper limit of a sum); the rows name every reading.
6. 13\|a: the head's **问题** still says the identifying assumptions are in §4.2 and that the group does not estimate; since the fold the extraction is part of this model. The restructure rewrites the head.
7. 5\|a: the false share to 8\|a and the import line of 8\|a (to `5|a|a|e|a`) are left to stage 4.
