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

## Stage 3: restructure

Date 2026-11-14, live box, under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; the chapter leads were written by hand). Backup: `D:\zt\backups\zt-before-ozhan2026monetary-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Moves ran as five `apply_batch` tree batches (5\|a: 14 items, one of them the create of the new chapter card; 8\|a: two batches of 39 and 40 items; 13\|a: 11 items; 19\|a: 8 items, two of them creates), each previewed, then committed under the plan hash; `links_that_will_break` was empty in every preview, and every commit applied on the first call. Text ran as `edit_card` calls under `-Allow`; before each text write a script compared the display formulas and the equation numbers (`\tag{…}`, `\quad (N)`) of the old and new bodies, and every written card was read back and compared with the intended text (0 differences). After the pass a script compared the whole book with the backup: 1,028 display formulas and 106 equation numbers before and after, no difference in either multiset (the eight display formulas of the four merged appendix cards were carried byte for byte into the cards they were merged into). No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 712 to 711 Cards: four new chapter cards (5\|a: 1; 8\|a: 1; 19\|a: 2) and five cards deleted `into` the card they restate (8\|a: 4; 13\|a: 1).

### 5\|a (formal): the intuition spine

```
5|a|a  设定 (setting part as the settings pass left it)
5|a|a|1  第 1 章：B 类任务受产能上限约束，部门供给曲线越过上限后陡升（§2.2.1） (new)   ← 5|a|a, 5|a|a|c
    a 结果：部门供给曲线 (7)   (proof a|a 约束不起作用 → 约束起作用 → (6) → (7), the former main-line 2–5;  a|b 图 1;  a|c 曲线移动)
5|a|a|2  第 2 章：商品部门的瓶颈经迂回投入抬高最终品价格，下游供给曲线分三段（§2.2.2）   ← 第 1 章, 5|a|a|d, d|a
    a 结果：两部门不受约束时 P=W (10)   (proof a|a)
    b 结果：商品约束下的最终品价格 (15)  (proof b|a (14) → (15))
    c 结果：下游供给曲线的三段           (proof c|a (11)–(13) → (16)–(18);  c|b 图 2)
5|a|a|3  第 3 章：市场出清与反馈规则闭合模型，陡峭的供给曲线使宽松只换来有限产出（§2.3）   ← 第 2 章, 5|a|a|e, e|a
    a 推导：欧拉方程 (3)
    b 结果：供给冲击下宽松的两难   (b|a 暂时冲击;  b|b 积极规则)
```

- The §2.2.1 derivations (former main-line 5\|a\|a\|2–5) left the main line into a new chapter card; the proposition became its result, with its four-step chain as the first Side branch and figure 1 and the curve shift after it. The flat Direct chain of chapter 2 was cut into its three propositions, each with its proof chain as a Side branch.
- Decision on the Euler equation (former 5\|a\|a\|1, §2.1): it went to chapter 3 as the first Side card, not to chapter 1. Its own **解读** says no later derivation of the upstream block uses (3) and that it serves the closure; it rests only on settings, so it stays a derivation without a result card.
- Result facets read `结果：…` (4 Cards; titles `命题：` → `结果：`). Every chapter card has **直觉** (lead of 20+ characters), **本章结果** and **承上启下** with `依赖` lines; chapters 2 and 3 keep their old 问题/概要/关联 text below (`本模型` → `本章`). The figure-2 card said `第 7 个模型`; it now says `第 3 章`.
- Notation card: the row `$\lambda_t$、$\lambda$` was split into two rows, because $\lambda$ (the multiplier of (8)) is now met first in 5\|a\|a\|2\|a\|a and $\lambda_t$ (the household multiplier) in 5\|a\|a\|3\|a.
- The Model Card: **结构** rebuilt (main line and one level under it); 概要 extended by one sentence on chapters 2–3; the two stale 对应 lines to its own chapters dropped; source now §2–§2.3. Three chapters, so no **分部**.

Network (`check-network.mjs`): 设定 → {1, 2, 3}.

### 8\|a (formal): the intuition spine

```
8|a|a   设定 (setting part as the settings pass left it)
8|a|a|1   第 1 章：约束起作用时价格而非数量吸收需求，政策面对跨期权衡（§1）   ← 8|a|a|b, 8|a|a
    a 机制 → a|1 结论 → a|1|a 数值例子 (unchanged)
8|a|a|2   第 2 章：家庭按工资、利率与财富选择劳动、债券与消费（§3.1） (new)   ← 8|a|a, d, c, c|a
    a 结果：劳动供给条件 (proof a|a);  b 结果：无抛补利率平价 (proof b|a);  c 结果：消费函数
    d 结果：对数偏好的消费倾向 (proof d|a);  e 结果：CRRA 下 Ω 的递归式 (proof e|a)
    f 结果：OLG 与习惯下 Ω 的递归式 (f|a 脚注 9);  g 结果：两层价格指数 (proof g|a)
8|a|a|3   第 3 章：劳动上限起作用时楔子抬高企业对劳动的估值（§3.2.1）   ← e, e|a, e|b
    a 结果：楔子与库恩—塔克条件 (20) (proof a|a)
    b 结果：楔子的取值 (21) (proof b|a, with A.2.2 merged;  b|b 含义)
8|a|a|4   第 4 章：国内与出口两条菲利普斯曲线同斜率，通胀预期可偏离理性预期（§3.2.2）   ← 第 3 章, f, f|a, f|a|a
    a 结果：国内与出口菲利普斯曲线 (proof a|a);  b 结果：预期偏离理性预期的程度 (proof b|a; b|b 校准)
8|a|a|5   第 5 章：泰勒规则、消除加成的补贴与三类市场出清闭合定量模型（§3.3–3.5）   ← 第 4 章, 第 2 章, g, g|a, g|b
    a 结果：单位补贴 (proof a|a);  b 结果：转移支付总额 (proof b|a);  c 结果：价格关系
    d 出清：产品市场;  e 出清：资产市场;  f 出清：劳动力市场 (22)
8|a|a|6   第 6 章：工资粘性、不完全劳动流动与固定资本使总体菲利普斯曲线更陡（§3.6）   ← 第 3 章, j, j|b, j|c
    a 结果：部门劳动供给 (proof a|a → a|a|1, with A.6 merged);  b 结果：固定资本与菲利普斯曲线 (proof b|a)
8|a|a|7   第 7 章：开放结构、工会与政策规则使部门冲击的总量效应偏离 Hulten 基准（§5.2）   ← 第 5 章, 第 6 章
8|a|a|8   第 8 章：部门劳动上限依次起作用，总体菲利普斯曲线在扩张段上翘并左移（§5.3）   ← 第 3 章, h
    a 机制 → a|1 结论 → a|1|a 例子 (unchanged)
8|a|a|9   第 9 章：世代交叠的家庭把工资、股息与国外资产折成财富（附录 A.1）   ← 第 2 章, i, i|a, i|a|a
    a 推导：劳动供给 (7.1-6);  b 推导：无抛补利率平价 (7.1-7);  c 定义：加总;  d 定义：两种财富与 Ω
    e 推导：劳动财富 (7.1-14);  f 结果：加总消费与财富成比例 (7.1-12) (proof f|a three steps);  g 推导：股息财富 (7.1-13)
8|a|a|10  第 10 章：工资、财富与净国外资产的总量方程闭合均衡（附录 A.1.1）   ← 第 9 章, i, j|a|a|a
    a–c 定义;  d 推导：Ω 的递推;  e–f 定义;  g 结果：净国外资产的累积;  h 结果：十四式闭合一般均衡
8|a|a|11  第 11 章：家庭与企业的需求把两层相对价格分开（附录 A.1.2、A.2.1）   ← 第 2 章, c, c|a, e|a
    a 定义;  b 结果：家庭的两层需求 (proof b|a);  c 结果：两级价格指数 (proof c|a two steps)
    d 结果：企业的两层中间品需求 (proof d|a);  e 推导：投入价格指数 → e|1 总投入价格
8|a|a|12  第 12 章：劳动上限只经有效工资进入劳动需求（附录 A.2.2）   ← 第 3 章, j|c
    a 结果：劳动需求只经有效工资 (proof a|a);  b 结果：资本与中间品需求 (proof b|a two steps);  c 结果：楔子抬高边际成本 (proof c|a)
8|a|a|13  第 13 章：Calvo 定价推出国内与出口的部门菲利普斯曲线（附录 A.3）   ← 第 4 章, f
    a, b 定义;  c 结果：国内与出口的实际价格 (proof c|a two steps);  d 结果：部门菲利普斯曲线 (proof d|a three steps)
8|a|a|14  第 14 章：国内线与出口线折算成总产出，费雪方程（附录 A.4–A.5）   ← 第 5 章, g
    a, b 定义;  c 结果：两条线折算成总产出 (proof c|a);  d 定义：费雪方程
```

(Chapter facets read `第 k 章：<short name>`; the titles above are shortened.)

- The household derivations of §3.1 (former main-line 8\|a\|a\|2–13) left the main line into a new chapter card 第 2 章; each `结果` card became a Side branch with its derivation as its proof. In chapters 3–6 the flat chains were cut the same way. The §1 verbal chapter and the §5.3 experiment keep their 机制/结论 chains.
- Decision on Appendix A: its derivations are not proof chains of the §3 results. They derive the blocks of the full model in the appendix notation and in the variant settings (the OLG households of variant one, the three-input technology with fixed capital of variant two), and several state forms the main text does not have (the UIP (7.1-7) carries an inflation ratio; the A.1.2 price index uses conditional shares, which the appendix itself sets apart from §3.1.2; the A.3 export curve does not divide marginal cost by the exchange rate). So the six appendix chapters stay chapters of their own (第 9–14 章, part four), the OLG form as its own variant chapter (第 9 章) with its aggregate block (第 10 章). The §3 result cards that state an appendix result now point to it: 2\|c (`附录的推导见第 9 章 [[…9|f]]`) and 2\|f (`附录的递推见第 10 章 [[…10|d]]`).
- Merged, because the appendix card restates a §3 derivation step for step (delete `into`, Links re-pointed; each was first moved out as a leaf, so its chain stayed in its chapter): the A.2.2 wedge derivation (former 23\|a) into `3|b|a`, the derivation of (21), which already writes (21) in the appendix notation $PZ_{d,j,t}$; the A.6 sector labour supply, wage index and their proposition (former 25\|a\|5, \|6, \|7) into `6|a|a`, `6|a|a|1` and `6|a`. Their eight display formulas went into the kept cards byte for byte, under a heading `附录 A.2.2 的写法` or `附录 A.6 的写法` with one sentence that names the notation. The A.6 chapter's stale **分部** section was removed (its third part pointed into the setting part).
- Result facets read `结果：…` (26 Cards; the former `结果` and `命题` facets; titles `命题：` → `结果：`). Every chapter card has **直觉**, **本章结果** and **承上启下** with `依赖` lines; the old 问题/概要/关联 text follows (`本模型` → `本章`; chapter 7 lost an empty **结构** heading; chapter 8 says `§3 的定量模型` for the paper's section). The 承上启下 line of chapter 6 first said `扩展设定`, which zt read as a `relaxes` relation; it now names the three settings on three `依赖` lines.
- Setting part, text only: `j|a|a` said `第 21 章的定义卡` (now `第 10 章`), `h` said `取自第 3 章` for the paper's §3 (now `§3 的`). No setting moved.
- The Model Card: new title (定量模型：两区域多部门网络中，劳动上限使价格而非产量吸收需求); 问题 widened from the household block to the whole model; one model-wide paragraph put before the old 概要 (now headed 家庭部门); **结构** rebuilt from the tree; **分部** added (设定与直觉 from 8\|a\|a; 家庭、企业与政策（§3） from 第 2 章; 数值实验（§5.2–5.3） from 第 7 章; 附录 A 的完整方程 from 第 9 章); the 关联 line that said the other parts have their own Model Cards and the source line were updated. The **引入** line to `5|a|a|e|a` is unchanged.

Network (`check-network.mjs`): 设定 → {1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14}; 6 → 7.

### 13\|a (empirical): the evidence spine

Main line: 数据 `13|a|a` → 假说：非对称供给传播 `|1` → 设计 `|2` → 规格：基准 `|3` (variant `|3|a`) → 第 1 章 参数取值 `|4` → 第 2 章 准非线性冲击提取 `|5` → 第 3 章 识别检验 `|6` → 第 4 章 准非线性反事实模拟 `|7` → 第 5 章 供给约束的贡献分解 `|8`. Network: 数据 → {假说, 设计}; 设计 → 规格 → {1, 2}; 2 → {3, 4}; 4 → 5.

- The hypothesis card (former `13|a|a|4|a`) moved to the main line before the design, as the skill's evidence spine asks; it rests on the data card only.
- Chapter 2 is the former §4.2 chapter card `13|a|a|4`. Its sub-chapter card `4|a|1` (第 1 章：提取的约束水平) held only a lead and a list of its estimates; its six children (图 15, 表 6, the four S02 estimates) moved under chapter 2, and the card was deleted `into` chapter 2, whose new **直觉** and **本章估计** carry its lead and list. The tests chapter (former `4|a|2`) became main-line chapter 3. The limits cards 预期 and 未报告拟合 and the former head-level limit 区域加总 (`13|a|b`) are the last Side branches of chapter 2 (`5|g`–`5|i`), each with one plain reference line (`见 […]`); loose 5 → 0.
- Chapters 4 and 5 (§4.3 counterfactual method, §6.2 decomposition) are kept as quantification chapters without estimate cards (their numbers are in the text and figures 10 and 14); each got **直觉**, **本章估计** (one line saying so) and **承上启下** with `依赖` lines to chapter 2 and the specification, or to chapter 4.
- **直觉** leads prepended to the data, hypothesis, design and baseline cards. Chapter numbers in text updated (`第 1 章` → `第 2 章` in the design's 假设 4–7 and the six estimate cards; `第 2 章` → `第 3 章` in the two tests).
- The Model Card: new title (拟合 2020–2023 年数据：两区域 11 部门的冲击提取、反事实与约束贡献); **问题** rewritten (the fit now includes the extraction, the counterfactual and the decomposition, and the identifying assumptions are 假设 4–7 of the design); **估计对象** no longer says the group does not estimate; **结构** rebuilt; source §4.1–§4.3, §6.2. No formula changed.

### 19\|a (formal)

Main line: 设定 `19|a|a` → 第 1 章：约束使受约束部门如弹性价格，按原权重盯住 DC 指数的规则收得过紧 `|1` (new; a 机制, b 结果：忽略约束的规则) → 第 2 章：把受约束部门的权重设为 0，约束期零缺口，解除后负缺口 `|2` (new; a 结果：按约束永久存在调整权重 with proof a|a 推导：受约束部门的权重; b 结果：约束解除后; c 政策含义, former `19|a|b`). Network: 设定 → {1, 2}. **结构** rebuilt.

### Gate

- `check-model.mjs 5|a`: 32 members (4 main, 27 side, 1 shared), no closure, forward or unanchored fault, loose 1, `self_sufficient` true (was false). The loose Link and the shared member are the false share to 8\|a's chapter 3 (`8|a|a|3`, the head's 对照 line), left to stage 4.
- `check-model.mjs 8|a`: 138 members (15 main, 122 side, 1 shared: the imported rule `5|a|a|e|a`), no fault, loose 0, `self_sufficient` true (was false).
- `check-model.mjs 13|a`: 40 members (9 main, 31 side), no fault, loose 0, `self_sufficient` true (was false).
- `check-model.mjs 19|a`: 12 members (3 main, 9 side), no fault, loose 0, `self_sufficient` true.
- `check-network.mjs`: `network: clean` for all four (advice only for 8\|a: chapters 1, 7 and 8 have no `结果：` card, right for the verbal chapter, the Hulten comparison and the §5.3 experiment).
- `check-premises.mjs`: 13\|a and 19\|a `premises: none used before their card`. 5\|a 1 hit, 8\|a 3 hits, all pointers, kept with this reason:
  - 5\|a: `5|a|a|2|b|a` remarks that the book's (16) also calls the rising term the goods input; it does not use (16).
  - 8\|a: the setting `8|a|a|e` (**直觉**, 怎样进入模型) says that the derivations of (20) and (21) start from (19), and `8|a|a|e|b|a` says that the derivation of (20) uses the assumption to separate $\lambda=0$ and $\lambda>0$; both point to the cards that use them, as after stage 2.
- `check-settings.mjs`: 5\|a, 8\|a, 19\|a `settings: all in the setting part, as a dependency tree` (8\|a advice: 9 settings under the setting card; `b` and `h` state one thing each); 13\|a `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` (agent): `{"broken":[]}`. Literature tree 712 → 711 Cards. Whole-book formula comparison with the backup: 1,028 display formulas and 106 equation numbers, no difference.
- Term relink: `{"links_added":133,"applied":true,"cards_changed":28,"book":"ozhan2026monetary","cards_refused":0,"links_repaired":0,"ok":true,"rounds":1}`. It linked the word 基准模型 in the **分部** line of 8\|a, which made the part line fail the parts check; the part was renamed `家庭、企业与政策（§3）`, and a second relink planned no change: `{"cards_refused":0,"rounds":0,"book":"ozhan2026monetary","links_repaired":0,"ok":true,"links_added":0,"cards_changed":0,"applied":false}`. All checks above, `list_broken_links` and the formula comparison were rerun after it and stay as listed.

### Open

1. Step 3 card text was not redrafted: result cards keep their digest body (no `**为什么**`/`**证明**` sections), derivation cards keep their digest format, and the old chapter cards keep their 问题/概要/关联 text below the new sections. The chapter-1 and chapter-8 cards of 8\|a keep the facets 机制/结论; 8\|a's appendix chapters keep the facets 定义 and 推导 for cards that end in no result.
2. 8\|a: the appendix chapters 9–14 still restate parts of §3 in the appendix notation (the OLG labour supply and UIP against chapter 2; the two-level demand and price indices against 2\|g; the A.3 Calvo curves against chapter 4; the A.4 output definitions against 5\|d). Their formulas differ from the main text, so they were kept; the import stage may decide whether one should cite the other. `12|a|a` (labour demand) still opens with `依赖上一步`, although its premise, the merged wedge derivation, now stands in chapter 3 (`3|b|a`).
3. 8\|a: the four stage-2 notation issues (two notations side by side, the A.6 CET exponent, the reading of $C$, $M$, $N$) stay; the A.6 formulas merged into chapter 6 carry the $\sum_{i\in N}$ notation.
4. 5\|a: the false share to `8|a|a|3` (head 对照 line) and the **引入** line of 8\|a to `5|a|a|e|a` are left to stage 4.
5. 13\|a: chapters 4 and 5 hold no estimate card; their figures and numbers stay in the chapter text.
6. Tree unlocked throughout; no `card_locked` and no read-view error met.

## Stage 4: imports and explanations

Date 2026-11-14, live box, under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent; one DeepSeek job, for the explanation refresh). Backup: `D:\zt\backups\zt-before-ozhan2026monetary-imports.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok; drive D had 223 GB free). Text ran as `edit_card` calls under `-Allow` (ozhan2026monetary Cards only): 9 Cards by hand (the Model Cards 5\|a and 13\|a, and 13\|a\|a\|1, \|3, \|3\|a, \|4\|b–\|4\|e), 1 import line on 8\|a by `imports.mjs apply`, and 1 Derivation part by `explain.mjs apply`. Before each hand write a script compared the display formulas and the `\tag`/`\quad (N)` numbers of the old and new bodies, and every written Card was read back (`same` on all nine). After the pass a script compared the whole book with the backup: 711 Cards before and after, 1,029 display formulas and 49 `\tag`/`\quad (N)` numbers identical as multisets, the 8 `值` lines of the estimate cards identical; 11 Cards changed in all. No `card_locked`, no `read_view_changed` and no failed batch occurred. Order: 5\|a (it lends to 8\|a), then 8\|a (it lends to 13\|a), 19\|a, 13\|a.

Before any edit, `check` exited 0 on all four heads; `plan` proposed one line (the `整块` upgrade on 8\|a) and nothing on 5\|a, 19\|a, 13\|a: 13\|a named the quantitative model only with `参照` Links and in words.

### 5\|a (formal, stylized model)

- No import. The model reads in itself; no part rests on another model's part.
- False share resolved by rewording, not by an import: the Model Card's **关联** line `对照 [[8|a|a|3]]：定量版用常替代弹性技术加劳动上限……` made 8\|a's chapter 3 a member (with its three settings 8\|a\|a\|e, e\|a, e\|b tolerated). Nothing in 5\|a rests on it: it compares two technologies of the same bottleneck. An import was also excluded because 8\|a imports 5\|a's feedback rule, so 5\|a importing from 8\|a would make the two models lend to each other. The line now reads `对照 [[ozhan2026monetary/8|a]]：定量模型（第 3 章）用常替代弹性技术加劳动上限，本卡用两项任务的柯布—道格拉斯技术。` (a Link between two Model Cards, no member). The reverse comparison stays where it was, as a part-level Link in 8\|a's chapter 3 (`对照 [[5|a]]`). False shares 1 → 0, tolerated 3 → 0, shared members 1 → 0 (32 → 31 members).
- Copy advice judged by hand, both comparisons, not copies: `5|a|a|3` (第 3 章, market clearing and the feedback rule) ~ `8|a|a|5` (8\|a's 第 5 章: Taylor rule, subsidy, three clearings) and ~ arrocannarsa2026flight/3\|e\|a\|5 (that model's five-market equilibrium). They are joined only by `对应` lines (`定量版政策与出清区块`; `跨书关联：对应 …两处都把最终品市场出清写成产出等于支出`); the equations differ (one-good clearing $Y^f_t=X_t+C_t$ against multi-sector two-region clearing). The advice stays in `check` output.
- 5\|a now lends `5|a|a|e|a` with its closure to 8\|a (`lent`).

### 8\|a (formal, quantitative model)

- Kept and upgraded: the fold's line to the stylized feedback rule. The Taylor rule `8|a|a|g|a` names it in **起点** (`依赖 [[5|a|a|e|a]]：程式化模型的反馈规则（经 **引入**），本规则是它的完整版本`). `plan` round 1 proposed `upgrade: 整块 [[5|a|a|e|a]]`; `apply`: `wrote 1 import line(s); tolerated 1 -> 0; check passes`; round 2 `nothing to import`. The line now reads `整块 [[ozhan2026monetary/5|a|a|e|a]]：程式化模型的反馈式利率规则，是本模型泰勒规则的简化版本`; its closure brings the stylized market clearing `5|a|a|e`, the setting card `5|a|a`, the downstream technology `5|a|a|d` and the assumption `5|a|a|b` (5 shared members; 142 members in all). No import cycle: 5\|a now imports nothing.
- Symbols (step 6): 8\|a keeps its own notation card; `check-settings` finds no symbol of the imported block uncovered, so no `沿用` line.
- Copy advice judged by hand: `8|a|a|4` (第 4 章, Calvo and DCP Phillips curves) and `8|a|a|13` (第 13 章, A.3) ~ arrocannarsa2026flight/3\|e\|a\|15 (that model's A.2 Calvo reset-price derivation). Joined only by `跨书关联：对应 … 两处都按 Calvo 规则重设价格`; the curves differ (two currencies of invoicing, sectoral slopes, expectation rules against one non-traded sector with indexation). A shared method, no restated block; the advice stays.
- 8\|a now lends the whole model to 13\|a.

### 19\|a (formal, DC-index weighting experiment)

- No import, decided. The model's setting card is a primitive setting of its own: a closed economy (item 5: stabilizing the DC index closes the output gap only in a closed economy with flexible wages and labour mobility), an **output** cap at steady-state output (`19|a|a|b`), and a rule on the DC index with weights $\mathrm{Domar}_{c,i}/\kappa_{c,i}$. 8\|a is a two-region open economy with a **labour** cap and a CPI Taylor rule; it does not define the DC index or the sectoral slopes $\kappa_{c,i}$, so no block of 8\|a is a premise of 19\|a. The PST weight (35%) and the sector calibration come from the book's calibration notes, which are no model part; the Model Card says so with `参照 [[ozhan2026monetary]]`. `19|a|a|b` item 4 (a labour cap gives qualitatively similar results) is a remark, not a premise.
- `check` exit 0, `plan` `nothing to import`, tolerated 0, false shares 0, no copy advice.

### 13\|a (empirical, fit of the quantitative model to 2020–23 data)

- Decision: an estimation of a theory imports the theory whole. The fit filters the data through the linearized quantitative model (state space $Y_t=Hs_t$, $s_t=Gs_{t-1}+\Psi\varepsilon_t$), extracts the multiplier of 8\|a's labour cap, takes the parameters of 8\|a's settings (Table 2) and runs the counterfactual in the same model. So the Model Card gained (before **结构**) `整块 [[ozhan2026monetary/8|a]]：本模型拟合的定量模型，状态空间所线性化的均衡方程组、劳动上限的乘子 (20) 与表 2 各参数所属的设定都取自它`, written by hand (`plan` proposes no whole-model line). 177 members (40 own + 137 shared).
- The parts now rest on the imported parts (one `依赖` line each, added after the existing ones; no estimate, no display formula changed):
  - baseline `13|a|a|3`: `依赖 [[8|a|a|10|h]]` (the full equilibrium system, 附录 A.1.1, whose linearization is the state space);
  - variant `13|a|a|3|a`: `依赖 [[8|a|a|3|a]]` ($\lambda^{L}$ is the multiplier of (20); the non-negativity and complementarity restrictions come from it);
  - parameters: `4|b` ($\gamma_c$) on `8|a|a|c` (two-layer CES); `4|c` ($\theta_c$) on `8|a|a|i` (OLG survival); `4|d` ($\gamma_L$) on `8|a|a|j|b` (CET labour (23), there $\gamma_l$); `4|e` ($\alpha_{c,\pi}$) on `8|a|a|g|a` (Taylor rule, there $\alpha^i_c$);
  - hypothesis `13|a|a|1`: `依赖 [[8|a|a|3|b]]：本假说检验的预测，楔子只在劳动上限起作用时为正 (21)，负向供给冲击因此经上限进入。` The `参照` lines to 8\|a's verbal chapter 1 and to 5\|a stay as pointers. The whole-model import covers the result, so no separate `单卡` line.
- Tolerated 0 → 1: `8|a|a|g|a` rests on `5|a|a|e|a`, which 8\|a imports; a whole-model import does not carry the lender's own imports, and `plan` proposes nothing. 13\|a uses no part of the stylized rule, so no line was added; a reader leaves the model there.
- Symbols (step 6): the 变量表 `13|a|a|a` keeps its rows; the imported notation card counts as the model's; `check-settings` clean, no `沿用` line.
- Copy advice `13|a|a|1` ~ `8|a|a|1` comes from the kept `参照` line; the hypothesis states the prediction, it does not restate the chapter. Left as advice.

### Cross-book blocks

`plan` proposed no cross-book line for any head, and no part of the four models rests on another book's Card. The cross-book Links are comparisons and stay: arrocannarsa2026flight/3\|e\|a\|5 and \|15 (judged above), arrocannarsa2026flight/8\|d, assenza2026louder/5\|b\|d\|1 and bergin2026exchange/5\|c\|b on 8\|a's chapter 3, bergin2026exchange/5\|g and furceri2026divide/4\|a on 13\|a (left for the evidence dialogue), atkinson2015lectures on 5\|a and 8\|a. No bergin2026exchange or arrocannarsa2026flight block is restated by these models: those are other open-economy models with their own settings.

### Explanation refresh (step 8)

- `explain.mjs plan`: 8\|a 1 Derivation part in 1 job (`8|a|a|12|a|a`, the labour demand of 第 12 章, whose position line `第 2 张，共 2 张` no longer matched its chain after stage 3); 5\|a, 19\|a, 13\|a `nothing to explain`.
- One DeepSeek conversation (`opencode-go/deepseek-v4.1-flash`, effort high, the box's drive model; OpenCode Go account Account 2, 17 requests, 440 s, 1 turn, context 115k of 1M; tools: 1 skill, 16 pwsh, 2 read, 1 write; only `out/explain-01.json` was written in the work folder; no approval was asked): answer `{"done":1,"blocked":0}`. Cost: OpenCode Go subscription, the Harness ledger books 0 CNY; the 2026-11-14 opencode-go day line went from 5,019 to 5,036 calls during the job (cache miss +114,777, cached read +1,159,424, output +31,378 tokens), matching the job's 17 requests. Conversation a0dcbf5a deleted, workspace removed. Conversation 3b1d187b was not touched.
- `apply`: `applied 1, refused 0, blocked 0, follow-up jobs 0`. `finish` 8\|a: `explained 1 part(s); Term links changed 0 Card(s), second plan 0; left 0; refused 0` (exit 0). The new **思路** drops the stale position line; it names no `假设`, so no Link had to be added. The part's **起点** still reads `依赖上一步 [[8|a|a|3|b|a]]` (explain leaves 起点 alone).

### Gate

- `imports.mjs check`: exit 0 on 5\|a, 8\|a, 19\|a, 13\|a; `plan`: `nothing to import` on all four.
- `get_model`: `self_sufficient: true` on all four; `false_share` empty on all four; no `import_cycle`, `import_target`, `closure`, `reach_in`, `forward` or `unanchored` fault; tolerated 0 on 5\|a, 8\|a, 19\|a and 1 on 13\|a (above). 8\|a imports `整块 5|a|a|e|a` (closure 4); 13\|a imports `整块 8|a` (137); 5\|a lends to 8\|a, 8\|a lends to 13\|a.
- `explain.mjs finish`: exit 0 on all four (`left 0; refused 0`); a later `plan`: `nothing to explain` on all four.
- `check-model.mjs`: 5\|a 31 members `{"main":4,"side":27}`, `relations {"refers":23,"rests-on":51,"corresponds":1,"illustrates":4,"contrasts":1}; loose 0; self_sufficient true`; 8\|a 142 `{"shared":5,"main":15,"side":122}`, `relations {"refers":114,"corresponds":19,"rests-on":221,"contrasts":7,"illustrates":4}; loose 0; self_sufficient true`; 19\|a 12 `{"main":3,"side":9}`, `relations {"refers":11,"rests-on":17}; loose 0; self_sufficient true`; 13\|a 177 `{"main":9,"side":31,"shared":137}`, `relations {"refers":165,"rests-on":257,"corresponds":20,"illustrates":8,"contrasts":7}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` on all four (advice only for 8\|a: chapters 7 and 8 have no `结果：` card, as at stage 3).
- `check-settings.mjs`: 5\|a, 8\|a, 19\|a `settings: all in the setting part, as a dependency tree` (advice only: `uses an assumption` lines on 5\|a\|a\|2 and on 8\|a\|a\|5\|c, \|6, \|13); 13\|a `settings: the data tree and the specification ladder are in place`.
- `check-premises.mjs`: 19\|a `premises: none used before their card`; 5\|a 1 hit and 8\|a 3 hits, the stage-3 pointers; 13\|a 4 hits: the same three pointers in the imported 8\|a setting cards (`8|a|a|e`, `8|a|a|e|b|a` name (20), (21)) and the 变量表 row of $\lambda^{L}_{i,t}$ (`式 (20) 的乘子`), a pointer to the imported block. Kept.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"applied":true,"book":"ozhan2026monetary","cards_changed":7,"rounds":1,"links_repaired":0,"cards_refused":0,"links_added":8,"ok":true}`. All checks above, `get_model` and the formula comparison with the backup were rerun after it and stay as listed.

### Open

1. 13\|a tolerates `5|a|a|e|a` through 8\|a's own import (a whole-model import does not carry the lender's imports). If the owner wants tolerated 0, 13\|a needs a second line `整块 [[5|a|a|e|a]]`, which it does not use.
2. 8\|a's `g|a` rests on the stylized rule as "its full version"; the quantitative Taylor rule responds to CPI inflation only and has no output-gap term, so the rest is closer to a comparison than a premise. Kept as the fold decided (owner-approved, stage 1).
3. `8|a|a|12|a|a` **起点** still says `依赖上一步 [[8|a|a|3|b|a]]` and step 2 says `代入第 1 张`, although the premise now stands in chapter 3 (stage-3 open item 2); the explanation refresh changes no 起点 or step.
4. Copy advice lines stay in `check` output, judged comparisons: 5\|a\|a\|3 ~ 8\|a\|a\|5 and ~ arrocannarsa2026flight/3\|e\|a\|5; 8\|a\|a\|4 and \|13 ~ arrocannarsa2026flight/3\|e\|a\|15; 13\|a\|a\|1 ~ 8\|a\|a\|1.
5. Stage-3 open items 1–5 (result and derivation text not redrafted; appendix chapters restating §3; the two notations; 13\|a chapters 4–5 without estimate cards) stay.
6. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.

## Stage 5: illustrate

Date 2026-11-15 (started late on 2026-11-14), live box (owner's request), under `zt-model-illustrate`, done in one side session by scripts. No subagent and no DeepSeek job ran, so there is no DeepSeek cost. A first attempt (2026-11-14) was cut off when its session ended: it took the backup `D:\zt\backups\zt-before-ozhan2026monetary-illustrate.sqlite3` but wrote no Card and left no module file; this run started clean. Backup of this run: `D:\zt\backups\zt-before-ozhan2026monetary-illustrate-2.sqlite3` (`VACUUM INTO`, 408 MB, `PRAGMA integrity_check` ok; drive D had 222 GB free). Only the three formal models were illustrated, each finished and written before the next: 5\|a, then 19\|a, then 8\|a. The empirical model 13\|a has no figures (owner, 2026-11-11). The imported block of 8\|a (`5|a|a|e|a` with its closure) belongs to 5\|a and was illustrated there. Modules (uncommitted, under `~/.claude`):

- `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\ozhan2026monetary-5_a.mjs`
- `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\ozhan2026monetary-19_a.mjs`
- `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\ozhan2026monetary-8_a.mjs`

Before each live write, every figure was rendered locally from the module (a copy of `verify.mjs` that reads the figures from the module, session scratchpad `local.mjs`) in light and dark, one PNG per part, and the PNGs were inspected. Faults fixed there: an x-axis label cut off at the bottom of the viewBox (5\|a\|a, 5\|a\|a\|e\|a), a cap label over an isoquant (5\|a\|a), a roundabout loop drawn through an upstream box (5\|a\|a\|d), a stray bracket over a heading (5\|a\|a\|e), a combining hat on ζ that rendered on the wrong letter (19\|a\|a\|b: the label is now in words, the caption keeps $\hat\zeta_{c,i,t}$ in TeX), labels over curves (19\|a\|a\|c, 19\|a timeline), `$` inside SVG text (`b^{$c}`, lint `svg-tex`; 8\|a\|a, f\|a, i\|a\|a: the currency superscripts are now words), an isoquant formula that left the plot (8\|a\|a\|e), bond labels wider than the gap between the two region boxes (8\|a\|a\|d), axis labels over notes (8\|a\|a\|g\|a, h), a cross over a box title (8\|a\|a\|i\|c), and the policy arrow of the 8\|a causal figure drawn through the wedge box. `apply.mjs --write` then wrote 8 Cards (5\|a), 4 Cards (19\|a) and 45 Cards (8\|a: the Model Card, 31 setting cards, 13 定义 parts), each `errors 0`. One more write changed the caption of `8|a|a|i|a|a` (see Chart fixes). No display formula, equation number or text outside the chart and the **直觉** section changed (apply guard). Every Model Card figure has three parts (`part()`, top-level `data-part`).

### Figures

- Model Card 5\|a (**图**, before **结构**): (一 · 主体与求解次序) six panels: the exogenous caps $\bar Y_t^j$ and the rigid wage $W$; the upstream sectors $g$, $s$ with tasks A and B (4) under the cap (5); the final-good firm (8) with $P_t^f=MC_t$; the household (1)(2); the central bank's feedback rule; market clearing $Y_t^f=X_t+C_t$; the solution order 第 1 章 (7) → 第 2 章 (10)(15) and the three segments → 第 3 章 (3) and the closure. (二 · 因果结构) caps, wage and rule parameters → upstream supply curve (7) → final-good price (10)(15) and the rule's rate → demand (Euler (3), clearing) → equilibrium output and inflation; dashed: roundabout production (the final-good price is an input price) and $\pi_t$, $x_t$ back to the rule. (三 · 模型研究的变化) seven panels: 第 1 章 crossing the cap, the negative supply shock; 第 2 章 the goods bottleneck in the final-good price (15), the three segments; 第 3 章 easing under a supply shock, the temporary shock, the active rule.
- 5\|a setting cards (7), at the end of **说明**: `5|a|a` (the household budget (2) as sources and uses; sector labour split over tasks A and B into $Y_t^j$ (4); the $(L_A, L_B)$ plane with the cap (5) on task B, the feasible set shaded and two isoquants, 示意); b (the flat path $W_t=W$ over periods; perfect foresight; the number line of employment $L_t\le\bar L$, 示意); c (one wage $W$ for both tasks; price taker, price = marginal cost; an exogenous cap path that tightens and recovers for $\bar Y^g_t$ and a flat $\bar Y^s_t$, 示意); d (goods, services and the final good itself → $Y^f_{i,t}$ (8), the roundabout loop, the share bar $\kappa_1$, $\kappa_2$, $1-\kappa_1-\kappa_2$, 示意); d\|a (each input's price source: goods capped, services uncapped, the final-good input at $P^f_t$ → $MC_t$ → $P^f_t=MC_t$, the common wage $W$); e (the split $Y_t^f=X_t+C_t$, 示意; the three actors; what the central bank looks at); e\|a (inertia part $\rho$ and response part $1-\rho$ with $\phi_\pi$, $\phi_x$ → $i_t$; $i_t$ against $\pi_t$ for two values of $\rho$, 示意).
- Model Card 19\|a: (一 · 时序与参与者) four panels (demand shock, output cap, firms and households, central bank with fixed DC weights) and a quarter timeline: the cap window (quarters 1–4), the PST demand path, the red rule (old PST weight) and the blue rule (PST weight 0) over 12 quarters; solution order 第 1 章 → 第 2 章. (二 · 因果结构) PST demand shock, PST output cap and the weight arrangement → shadow marginal cost $MC=\tilde{MC}+\lambda^Z$ and PST as flexible-price ($\kappa\to\infty$) → PST relative price, DC index → policy stance → output gap; dashed feedback of demand to PST. (三 · 模型研究的变化) six panels: 第 1 章 the mechanism and the rule that ignores the cap; 第 2 章 the weight 0 (2\|a\|a), zero gap during the cap, the negative gap after it lifts, the policy implication.
- 19\|a setting cards (3): `19|a|a` (a sector network, 示意; Calvo slopes $\kappa_{c,i}$; Domar weight ÷ slope = $\omega^D_{c,i}$; the DC index with PST 35%, the other segments 示意; the fixed-weight rule); b (the number line of PST output with the cap at steady-state output; $MC_{c,i,t}=\tilde{MC}_{c,i,t}+\lambda^Z_{c,i,t}$ as two segments, 示意); c (the PST demand path: +10% for quarters 1–4, then AR(1) 0.8, computed; the cap window shaded; other sectors' proportional fall, 示意).
- Model Card 8\|a: (一 · 参与者与次序) five panels in the order of one period (shocks; central bank and subsidy; sector firms under the cap with Calvo and DCP; households; three markets) and the solution order by chapter (2 → 3 → 4 → 5, 6, 7–8, 9–14 appendix, 1 verbal). (二 · 因果结构) labour cap, preference shock, policy shock, stickiness and elasticities → sector labour demand → wedge $\lambda_{c,i,t}$ (20)(21) → marginal cost → sector Phillips curves → sector and CPI inflation; policy → rate and UIP → consumption → labour demand; output and employment; dashed: CPI inflation back to the Taylor rule. (三 · 模型研究的变化) nine panels: 第 1 章, 第 3 章, 第 4 章 (two), 第 5 章, 第 6 章, 第 7 章, 第 8 章, 第 12 章.
- 8\|a setting cards (31): `8|a|a` (two regions × $S$ sectors; the household split $\mu_c$ / $1-\mu_c$, 示意; the Ricardian budget as sources and uses); b (US and ROW × 11 sectors; the cap slack or binding; the two information states of the DC rule); c (the two-layer CES tree, $\gamma_c$ and $\gamma_i$; the within-period order); c\|a (steady share × preference shock ÷ sum; shares before and after a shock to sector 1, 示意); d (two regions, global bonds, no labour across regions; habit and prices taken as given); e ((19) as two inputs; two isoquants, easy and hard substitution, 示意; observe then choose); e\|a (the intermediate tree, $\gamma_m$ and $\gamma_i$; the number line $\gamma_m<\gamma_i$, 示意); e\|b (the cap $\hat x_{c,i,t}\bar L_{c,i}$ over time, the hireable region, 示意); e\|b\|a (labour demand below the cap in normal times, pressed to the cap in a crisis, 示意; $\lambda=0$ against $\lambda>0$); f (the Calvo reset share, 示意; the number line of $\delta_{c,i}$; one curve or two by invoicing currency); f\|a (the curve as expectation + slope × cost gap; the slope against $\delta_{c,i}$ computed with $\beta_c\theta_c=0.99$, 示意); f\|a\|a (three-term expectations, 示意; the RE case; domestic and export rules); g (government, firms, central bank; the order in a period; three markets); g\|a (the rule's three parts; the calibrated response lines, US $\alpha=3$, ROW $1.5$, $\rho=0.5$); g\|b (price = marginal cost + markup without and with the subsidy, 示意; households → government → firms); g\|c (two NFA bars summing to zero, 示意); g\|c\|a ($P^C_{US,t}=1$ → $P^C_{c,t}=1/\mathcal E_{c,t}$; relative against absolute prices); h (caps by sector: sector 1 at steady state, sector 10 +1.75%, sector 11 unconstrained, sectors 2–9 interpolated, 示意; the policy-stance number line); i (cohort shares $\iota_c/n_c\cdot(\theta_c/n_c)^a$ with illustrative values; population growth); i\|a (the weights $(\beta_c\theta_c)^s$, 示意; the period utility's blocks); i\|a\|a (the OLG budget as resources and uses); i\|a\|a\|a (a discounted debt path that vanishes and one that does not, 示意; $\mathcal H\to\mathcal F\to\mathcal J$); i\|b (parallel log-consumption lines of three cohorts, 示意); i\|c ($(1-\mu_c)(WL+G)$ → HtM consumption, no assets); j (unions and firms per sector; the two household types); j\|a (the union reset share, 示意; the wage Phillips curve; the wage gap); j\|a\|a (the aggregate wage curve; the MRS as $C/(\bar L-L)$ times $(1-\eta)/\eta$; the real wage); j\|a\|a\|a (three-term wage expectations, 示意); j\|b (CET level sets in the plane of two sectors' labour for small and large $\gamma_l$, 示意); j\|b\|a (hours by type summing to total labour; CET to sectors); j\|c (three inputs with fixed capital; the appendix order of observe and choose).

### 直觉

The **直觉** section was rewritten on all 41 setting cards (5\|a: 7; 19\|a: 3; 8\|a: 31) and written on the 13 定义 parts of 8\|a (`9|c`, `9|d`, `10|a`, `10|b`, `10|c`, `10|e`, `10|f`, `11|a`, `13|a`, `13|b`, `14|a`, `14|b`, `14|d`); every one had a digest or stage-2 section. The old sections carried Links (5\|a\|a), `（补：书中未写）` marks (5\|a\|a, b, c, e\|a; 8\|a\|a\|b, g, g\|a, g\|b, g\|c, g\|c\|a, i\|a\|a, i\|c, j\|a\|a, j\|a\|a\|a and most 定义 parts), names of facets or cards (`分面「推导：边际成本」`, `“结果：单位补贴”卡`, `第二部分`), later equation numbers ((20), (21)) and a result (5\|a\|a: the convex supply curve). The new sections name chapters in words (第 1–3 章; 第 1–2 章; 第 1–14 章), with no Link, no result, no 依赖 line and no assumption name; setting cards use $TeX$ only for symbols the card or an earlier setting defines. `check-premises` on 8\|a fell from 3 hits to 0 (the pointers to (20) and (21) in the **直觉** of `e` and `e|b|a` are gone).

### Chart fixes outside the setting part

None. The three models had no digest chart (the book's figures 1 and 2 are artifact images). One fix inside the setting part, after the first write: `check-settings` on 8\|a reported `uncovered: H|a|i|a|a uses $v$ $g$ $e$ $w$ $B$ $x$ $m$ $l$ …`. The card's **设定** writes `$b_{c,a,t}^{\$}$`; the escaped `\$` shifts the check's `$`-pairing, so the two TeX formulas of the new caption made it read SVG and caption text as a formula. The caption of `8|a|a|i|a|a` now names the terms in words (no `$`), and a comment in the module says why; the check is clean again. The card's own formula was not changed.

### Gate

All lines below are the reruns after the Term relink.

- `verify.mjs --model ozhan2026monetary/5|a` (light and dark): `8 chart(s) in 32 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets.
- `verify.mjs --model ozhan2026monetary/19|a`: `4 chart(s) in 13 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets.
- `verify.mjs --model ozhan2026monetary/8|a`: `37 chart(s) in 143 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets (the count includes the five charts of the imported 5\|a cards, linted as parts).
- `apply.mjs` rerun: `same` for all 8 + 4 + 45 Cards.
- `check-settings.mjs`: 5\|a, 19\|a, 8\|a `settings: all in the setting part, as a dependency tree` (8\|a advice and balance lines as at stage 4: 9 settings under the setting card, `b` and `h` state one thing, the `uses an assumption` lines); 13\|a `settings: the data tree and the specification ladder are in place`.
- `check-model.mjs`: 5\|a `members 31 {"main":4,"side":27}`, `relations {"refers":21,"rests-on":51,"corresponds":1,"illustrates":4,"contrasts":1}; loose 0; self_sufficient true` (refers 23 → 21: the old **直觉** of `5|a|a` linked `b` and `c`); 19\|a `members 12 {"main":3,"side":9}`, `relations {"refers":11,"rests-on":17}; loose 0; self_sufficient true`; 8\|a `members 142 {"shared":5,"main":15,"side":122}`, `relations {"refers":113,"corresponds":19,"rests-on":221,"contrasts":7,"illustrates":4}; loose 0; self_sufficient true`; 13\|a (imports 8\|a whole) `members 177 {"main":9,"side":31,"shared":137}`, `relations {"refers":165,"rests-on":257,"corresponds":20,"illustrates":8,"contrasts":7}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` on 5\|a, 19\|a, 8\|a (8\|a advice: chapter 8 has no `结果：` card, as before).
- `check-premises.mjs`: 5\|a 1 hit, the stage-3 pointer in `5|a|a|2|b|a` to (16); 19\|a and 8\|a `premises: none used before their card`.
- `zt-check --cards` on the 57 written Cards: `faults: 14`, all in existing text: 5\|a\|a **说明** (R3); 19\|a\|a **起点** and **说明** (R3 ×2); 8\|a\|a **设定** and **说明** (R2, R3 ×3); 8\|a\|a\|c (R2, R3), e\|a, f, i, j (R3) in **设定**/**起点**; 8\|a\|a\|i\|a\|a **设定** (R2). None is in a 直觉 section or a chart.
- `list_broken_links` (agent): `{"broken":[]}`, before and after the relink.
- Term relink: `{"cards_changed":31,"cards_refused":0,"book":"ozhan2026monetary","links_repaired":0,"ok":true,"applied":true,"links_added":48,"rounds":1}`; `-PlanOnly` afterwards: `{"links_repaired":0,"cards_changed":0,"applied":false,"book":"ozhan2026monetary","ok":true,"rounds":0,"cards_refused":0,"links_added":0}`.

### Open

1. Stage 7 (literature organize) will renumber addresses. zt 0.63+ rewrites chart `data-zt` targets with the moves, but each module's `head` (`ozhan2026monetary/5|a`, `19|a`, `8|a`, and with it every `A()` address built from it) must be changed to the model's new address after the organize; then run `apply.mjs --write` on each new head and `verify.mjs --model` on it, which must show 0 dead targets and `same` on a second `apply.mjs`. `apply.mjs` refuses a head that is no longer a Model Card.
2. `8|a|a|i|a|a` **设定** writes `$b_{c,a,t}^{\$}$`. The escaped dollar shifts check-settings' `$`-pairing, so any later TeX added to that card can make the check read text as formula symbols; the caption there stays in words. A rewrite of the symbol (for example `b^{\mathrm{USD}}`) would remove the trap but changes a formula, so it waits for the owner.
3. The 14 zt-check faults (R2/R3 in existing **起点**/**设定**/**说明** text) predate this pass and were not changed.
4. Several figures draw shapes or values the source does not fix (shares, paths, the slope's $\beta_c\theta_c=0.99$, the cohort shares' $\iota_c=0.06$, $\theta_c=0.95$, the interpolated caps of sectors 2–9); each caption says 示意.
5. The stage-4 open items stay.
6. No `card_locked`, no `read_view_changed` and no failed write. Other sessions may have written other books on the box at the same time. Nothing committed (stage brief).
