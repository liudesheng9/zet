# riboni2026presidential: post-digest model passes

Riboni and Ruge-Murcia, *Presidential Appointments to the Federal Reserve Board*, Work 29956. The digest is settled with one gap the owner accepted (audit/book@3); see `D:\zt-digest\_queue\imen1-queue.md`. All passes ran on the live box (`D:\zt`), 2026-11-14.

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-riboni2026presidential-fold.sqlite3` (integrity_check ok).

The survey found 11 Model Cards (1 concept, 10 empirical) in 11 candidate groups: no rests-on Link joined any two. Two rows needed judgement (8\|b and 9\|a). The proposal was shown to the owner, who approved it on 2026-11-14 ("Yes, fold as proposed").

| Model Card | Level | Model | Surviving head | Reason |
|---|---|---|---|---|
| 4\|a | empirical | appointment effect (LP-IV) | 4\|a | §3.1–3.2: the estimand (effect of the governors' share G on outcomes h meetings later), local projections with 2SLS and four external instruments |
| 3\|a | concept | appointment effect | 4\|a | the three-valued coding of the policy variable: the measurement built for this model's outcome |
| 3\|b | empirical | appointment effect | 4\|a | the meeting-level data (391 regular FOMC meetings) the model uses |
| 5\|a | empirical | appointment effect | 4\|a | the baseline result of the same estimand, data and instruments |
| 5\|a\|4 | empirical | appointment effect | 4\|a | heterogeneity: the share split by the appointing party |
| 6\|a | empirical | appointment effect | 4\|a | heterogeneity: appointments joining in election years |
| 7\|a | empirical | appointment effect | 4\|a | heterogeneity: interaction with the president's party |
| 8\|a | empirical | appointment effect | 4\|a | mechanism: the same instruments, the outcome is the members' policy stance |
| 8\|c | empirical | appointment effect | 4\|a | mechanism: the same instruments, the outcome is dissents |
| 9\|a | empirical | appointment effect | 4\|a | §5: the same appointment shock on the same meetings, estimated with another estimator family (SVAR-IV, departures as external instrument) on inflation and output |
| 8\|b | empirical | reaction function | 8\|b | §4.2: a different estimand (the association of stated stance with the same meeting's decision, no instrument); it takes the stance measure from 8\|a |

The book holds **2 models**: `4|a` and `8|b`, both empirical.

### Fold (executed 2026-11-14, live box)

Three runs of the fold script, re-reading the tree between runs (a fold removes a Side slot and renames later siblings, so later siblings were folded first):

- Main line of 4\|a after the fold: `4|a|a` data; `4|a|a|1` 数据章：三值政策变量的编码 (was 3\|a); `|2` 数据章：会议层面数据与政策变量 (was 3\|b); `|3` design; `|4` baseline specification; `|5`–`|6` the existing chapters 1–2; `|7` 第 3 章：基准局部 IV 投影 (was 5\|a); `|8` 第 4 章：党派分组的局部投影 (was 5\|a\|4); `|9` 第 5 章：选举年的局部 IV 投影 (was 6\|a); `|10` 第 6 章：与总统党派的交互 (was 7\|a); `|11` 第 7 章：政策立场的局部 IV 投影 (was 8\|a); `|12` 第 8 章：异议的局部投影 (was 8\|c); `|13` 第 9 章：SVAR-IV 与宏观结果 (was 9\|a).
- Each folded card's data and limitation parts are filed under it. Idea notes that were Direct successors of a folded card (5\|a\|1–5, 6\|a\|1–3, 7\|a\|1–3) moved to their section as Side branches; the idea Side cards 3\|a\|a and 6\|a\|a stay in their sections.

### Gate

- `model-survey.mjs`: exactly 2 Model Cards, 4\|a (185 parts) and 8\|b (25 parts). One `false share` from 8\|b to a part of 4\|a (its Link to the stance model, former 8\|a): left to the import stage.
- `check-model.mjs 8|b`: 25 members (1 shared), `self_sufficient` true. `check-model.mjs 4|a`: every former head is a part; the remaining faults are the unanchored folded chapters (`4|a|a|1`, `|2`, `|7`–`|13`) and 9 loose parts, which the settings and restructure passes fix.
- `list_broken_links`: empty. `term-relink.ps1`: `{"links_added":9,"cards_changed":6,"ok":true}`.

### Open

- Every folded chapter carries its own data card, restating the 391 meetings; the settings pass merges them into one data tree.
- 8\|b's Link to the stance measure becomes an **引入** line in stage 4.

## Stage 2: settings (strict)

Date 2026-11-14, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-riboni2026presidential-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The moves and deletes ran as two `apply_batch` tree batches (50 and 46 items; preview, then commit under the plan hash; `links_that_will_break` empty in both). Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the `\tag` numbers of the old and new bodies, and every written card was read back and compared with the intended text (0 differences). No display formula and no equation number changed in a card that stays. No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 514 Cards to 460: 4\|a lost 54 Cards (185 parts to 131); no card was created.

### 4\|a (empirical)

Each of the nine folded chapters carried its own build: a data card restating the 391 meetings, a 变量表, a design with two to four assumptions, one or more specifications and two or three sub-chapters of estimates. The two former data chapters (`4|a|a|1` coding of the policy variable, `4|a|a|2` meeting-level data) stood on the main line. All of it was dissolved into one data tree, one design and one specification ladder. Addresses below are those before the pass unless marked "now".

- **Deleted `into` the card each restates** (Links re-pointed): the data cards 2\|a, 7\|a, 8\|a, 9\|a, 10\|a, 13\|a into the data card `4|a|a`; 11\|a into the stance measure and 12\|a into the dissent measure; the eight 变量表 (2\|a\|a, 7\|a\|a … 13\|a\|a) into `4|a|a|a`; the eight designs (2\|a\|1, 7\|a\|1 … 13\|a\|1) into the design; the exclusion restatements 7\|a\|1\|a, 8\|a\|1\|a, 9\|a\|1\|a, 10\|a\|1\|b, 11\|a\|1\|a, 12\|a\|1\|b, 12\|a\|1\|c into 假设 5; the strength restatements 7\|a\|1\|b, 8\|a\|1\|b, 9\|a\|1\|c, 11\|a\|1\|b, 12\|a\|1\|a, 13\|a\|1\|b into 假设 6; 9\|a\|1\|b into 假设 4; 13\|a\|1\|a into 假设 1; 2\|a\|1\|a (every meeting has a Greenbook) into the data card's sample rule; 13\|a\|c into the instruments measure; the former chapter 4\|a\|a\|1 with its cards 1\|a, 1\|a\|1, 1\|a\|2, and the coding rule 2\|a\|2, into the policy measure; the sample-size estimate 2\|a\|3\|a and the roadmap chapter 2\|a\|3 into the data card; the variable-choice test 2\|a\|3\|b\|a into the correlation card; the former chapter 4\|a\|a\|2 into the data card.
- **Deleted duplicates of the ladder's equations.** The digest had written each source regression twice. The pass kept the copy that carries the source's equation number and deleted the other: `4|a|a|4` (the untagged copy of (1)) into the tagged baseline (former 7\|a\|2); `4|a|a|4|a` (a copy of (2) written $G^{R}_t,G^{D}_t,\theta$) and 7\|a\|2\|a (a copy of (2) with a shared $\kappa_0$–$\kappa_3$ first stage) into the former 8\|a\|2, which writes (2) with its own first stages; `4|a|a|4|b` (a generic $\gamma^{h}G_tZ'_t$ interaction stated in §3.1) into the election-year specification (3). The 变量表 row $Z'$ went with it; the §3.1 remark that the source does not say whether the product is endogenous moved to the **说明** of (3).
- **Moved, because no card held them:** the measures of the policy variable, the Greenbook controls, members and dissents (from 2\|a), of the VAR's macro series (13\|a\|b), of dissent ratios (12\|a\|b) and of stance (11\|a\|b, with its sample 11\|a\|c) into the data tree; the correlation 0.86 (2\|a\|3\|b), retyped as `描述：` under the Greenbook measure; seven new identifying assumptions under the design (假设 7–13 below); the specifications of chapters 3–9 onto the ladder; the sub-chapters of each folded chapter (7\|a\|3–4 … 13\|a\|3–5) up into their chapter's first Side slot, ahead of the chapter's limits card; the limits card of the former coding chapter (2\|b) to the Model Card as `4|a|c`.

The setting part now reads:

```
4|a|a    数据：391 次例行 FOMC 会议                                原始设定
├ a  变量表 (one table, 50 rows; one row per written form where the source uses two notations)
├ b  测量：理事占比 G_t, party shares G^R/G^D (= R_t, D_t), A_t, g_t, president party P^p
├ c  测量：四个外部工具变量 Z_t (z_t, E_t, D^j)
├ d  测量：三值政策变量 Policy_t = Tight_t - Ease_t (merged from former 4|a|a|1, 1|a, 1|a|1, 1|a|2, 2|a|2)
├ e  测量：绿皮书实时数据 X_{t-1}
│    e|a 描述：工业生产与产出增长 (0.86)
├ f  测量：VAR 的宏观序列 x^{in}, x^{og}, x^{ei}, x^{eo}
├ g  测量：成员与异议
├ h  测量：异议比例 D^R, D^D (262 dissents, composition)
└ i  测量：政策立场 (4)
     i|a 样本：有逐字记录的会议 (17 meetings dropped)
4|a|a|1  设计 (one design: variation, seven comparisons, threats of every former design)
├ a 背景；b–g 假设 1–6 (instruments, exclusion, strength)
├ h 假设 7 三值方向可比 (2|a|1|b); i 假设 8 会议开始时的利率目标 (2|a|1|c); j 假设 9 绿皮书无断裂 (2|a|1|d)
└ k 假设 10 三值符号可靠 (7|a|1|c); l 假设 11 党派系数分开解读 (8|a|1|c);
  m 假设 12 反对党任命者的离任 (10|a|1|a); n 假设 13 任命只经政策决策影响经济结果 (13|a|1|c)
4|a|a|2  规格：基准 (1) LP-2SLS (from 7|a|2)
├ a 党派分组 (2) (from 8|a|2)
├ b 选举年交互 (3) (from 9|a|2): a 非选举年; b 共和党选举年 → b|a 非选举年; c 民主党选举年 → c|a 非选举年;
│   d 总统党派交互第 1 列 (from 10|a|2) → d|a 第 2 列, d|b 第 3 列
├ c 结果：共和党任命者鸽派立场 (from 11|a|2) → a 鹰派; b 民主党鸽派 → b|a 鹰派
├ d 结果：共和党任命者异议比例 (from 12|a|2) → a 民主党任命者
└ e SVAR-IV (from 13|a|2): a 预期通胀, b 通胀, c 预期产出增长, d 产出增长
```

The main line is now data, design, baseline, then chapters 1–9 at `4|a|a|3`–`11`.

- **Statements moved and pointers.** The data card took the sources of every restating data card (Thornton 2005, Thornton and Wheelock 2014, the Fed history site) and a sub-sample line for the stance and dissent outcomes. The share measure took the party split, the president-party dummy from Table 1 and the alternative notations the specifications use. The policy measure took the three instrument regimes, the coding rule, the footnote-7 reasons and the **直觉** of the former setting card, and kept both cross-book Links (assenza2026louder, bergin2026exchange). The design took the comparisons and threats of the eight sub-designs. 假设 1 took the narrative evidence of the SVAR design; 假设 5 states that the exclusion restriction covers every outcome; 假设 6 took the per-horizon testing, the relevance figures (177 and 266 days, 0.35) and the SVAR's Montiel Olea–Stock–Watson Wald test. The renumbered assumptions 7–13 got new facets, titles and body numbers, and their named cross-references are now Links.
- **Ladder text.** Every variant rests on the specification it changes by a `依赖` line and states **相对基准的改动**; the moved specifications got facets that name their variant (`规格：党派分组 (2)`, `规格：选举年交互 (3)`, `规格：总统党派交互（第 1–3 列）`, `规格：异议（…）`, `规格：SVAR-IV`). The empty 各项 list of the Democratic dissent specification was filled.
- **补写 equations added.** The SVAR-IV variants 2\|e\|a, b and c (expected inflation, inflation, expected output growth) stated only the VAR. Each now also states the first stage $u^{d}_t=\lambda z_t+\eta_t$ and the response ratio $\theta$, as 补写 copied from the baseline, under the line `原书没有写出本规格的第一阶段（工具变量）方程与响应。本卡按基准 SVAR-IV 规格补写`. Every other specification already stated both stages.

The **结构** section of the Model Card was rebuilt from the build order (all parts except estimates, tests and figures).

### 8\|b (empirical)

The measure `8|b|a|b` got a one-line summary, **起点** and **内容**. Both specifications are OLS with their equations already written (补写 by the digest); no instrument, so no first stage is due. The Link from the Model Card to chapter 7 of 4\|a (the shared part) was left for the import stage.

### Gate

- `check-settings.mjs riboni2026presidential/4|a`: `settings: the data tree and the specification ladder are in place`.
- `check-settings.mjs riboni2026presidential/8|b`: `settings: the data tree and the specification ladder are in place`.
- `check-model.mjs 4|a`: 131 members (12 main, 117 side, 2 off), loose 8; `unanchored` on chapters 3–9 (`4|a|a|5`–`11`), so `self_sufficient` stays false. Left to the restructure, as after stage 1.
- `check-model.mjs 8|b`: 25 members (1 shared), loose 1, `self_sufficient` true.
- `list_broken_links` (agent): empty, before and after the relink.
- Term relink: `{"ok":true,"links_repaired":0,"links_added":80,"book":"riboni2026presidential","cards_refused":0,"rounds":1,"cards_changed":22,"applied":true}`. Both settings checks and `list_broken_links` were rerun after it and stay clean.

### Open

1. 4\|a: chapters 3–9 are unanchored and 8 parts are loose; the restructure builds the evidence spine.
2. 4\|a: chapter 3 (`4|a|a|5`) holds a sub-chapter on the party split (`5|a|1`, two estimates) that restates the estimates of chapter 4 (`6|a|b`, `6|a|c`); the weak-instrument test of chapter 2 (`4|a|a|4|a`) and of chapter 4 (`6|a|1|a`) have the same title. The restructure decides the merge.
3. 4\|a: the specifications keep the digest's several notations for one object ($G^{R}_t$ and $R_t$; $A_t$; $g_t$ and $d_t$ in the SVAR; the letter $D$ for the Democratic share, the election-timing dummy $D^{j}_t$ and the dissent ratios $D^{R}_{t+h}$), because no display formula may change. The 变量表 gives each written form its row and names the others.
4. 4\|a: the president-party specifications (`2|b|d`, `d|a`, `d|b`) name their dependent variable 政策立场 in 各项, while their equation and the former 变量表 measure it by the three-valued policy variable; the wording was not changed and should be checked against §3.5.
5. 8\|b: the Link to chapter 7 of 4\|a becomes an **引入** line in stage 4.

## Stage 3: restructure

Date 2026-11-14, live box, under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; the chapter leads were written by hand). Backup: `D:\zt\backups\zt-before-riboni2026presidential-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The moves and merges ran as one `apply_batch` tree batch of 62 items (37 moves, 18 deletes `into`, 7 reorders of chapter Side branches), previewed, then committed under the plan hash; `links_that_will_break` was empty. Text ran as 48 `edit_card` calls under `-Allow` (31 in 4|a, 8 in 8|b, 7 chapter titles, 2 **结构** sections rebuilt last)\|a, 8 in 8\|b, 7 chapter titles, 2 **结构** sections, 1 re-run of the 4\|a **结构** after the titles); every written card was read back and compared with the intended text (0 differences). Before each text write a script compared the display formulas and the `\tag` numbers of the old and new bodies. After the pass a script compared the whole book with the backup: 329 display formulas and `\tag` numbers before and after (302 distinct), none lost or added, and no card at an unchanged address changed a formula. No estimate number was moved or rewritten. No `card_locked` error, no `read_view_changed` error and no failed batch occurred. The Literature tree went from 460 to 442 Cards (18 deleted `into` the cards that hold their content; none created). The setting parts kept their structure; the only edits there are a prepended **直觉** lead on the data, design and baseline specification cards, the outcome words of the three president-party specifications (below) and one chapter number in 8\|b's 假设 3.

### 4\|a (empirical): the evidence spine

```
4|a|a    数据                                            (data tree as stage 2 left it)
4|a|a|1  设计                                            (design and 假设 1–13)
4|a|a|2  规格：基准 (1)                                   (specification ladder)
4|a|a|3  第 1 章：确认延迟与极化提供了第一阶段的证据 (§3.2)   ← 数据, 规格
4|a|a|4  第 2 章：弱工具变量检验 (§3.2)                    ← 第 1 章
    a 检验：弱工具变量 (Lewis–Mertens, all 26 projections)
4|a|a|5  第 3 章：增任理事先带来扩张，15 次会议后反转 (§3.3)  ← 规格 (1), 第 2 章
    a 图 2; b 估计：先扩张; c 估计：反转; d 局限
4|a|a|6  第 4 章：共和党与民主党任命者的效应几乎相同 (§3.3)  ← 规格 (2), 第 3 章
    a 图 2 (B, C); b 估计：共和党任命者 → b|a 检验：弱工具变量 (2); c 估计：民主党任命者; d 局限
4|a|a|7  第 5 章：选举年加入的任命预示紧缩 (§3.4)          ← 规格 (3), 第 3 章
    a 图 3; b 非选举年; c 选举年 → c|a 检验; d–g 按党派的四条路径; h 局限
4|a|a|8  第 6 章：控制总统党派后选举周期不变 (§3.5)        ← 规格 2|b|d, 第 5 章
    a 图 4; b 非选举年; c 选举年; d 共和党任命者 → d|a 检验：反对党任命者的离任; e 民主党任命者; f 局限
4|a|a|9  第 7 章：任命使同党任命者的立场更鸽派 (§4.1)      ← 规格 2|c, 第 4 章
    a 图 5 (A, B, D, E); b 共和党鸽派 → b|a 检验; c 共和党鹰派; d 民主党鸽派; e 民主党鹰派; f 局限
4|a|a|10 第 8 章：任命增加扩张方向的异议 (§4.3)            ← 规格 2|d, 第 4 章
    a 图 5 (C, F); b 共和党任命者的异议 → b|a 检验：弱工具变量, b|b 检验：离任原因; c 民主党任命者的异议; d 局限
4|a|a|11 第 9 章：任命冲击推高通胀与预期通胀，对产出增长没有显著效应 (§5)  ← 规格 2|e, 第 3 章
    a 估计：政策决策 → a|a 检验：Wald 弱工具, a|b 检验：离任原因, a|c 检验：与局部投影对照
    b 预期通胀 → b|a 图 6; c 通胀 → c|a 图 7; d 预期产出增长; e 产出增长; f 局限
4|a|b, 4|a|c  局限 (Model Card side, unchanged place)
```

Network (`check-network.mjs`): 数据 → {设计, 规格}; 规格 → 第 1 章 → 第 2 章 → 第 3 章; chapters 4–9 hang under the specification block, because their estimates rest on the ladder's variants more often than on the chapter they extend. **分部** (new in the Model Card): 数据与识别 from `4|a|a`; 政策决策 from 第 3 章; 机制 from 第 7 章; 宏观结果 from 第 9 章.

- **The folded sub-chapters were dissolved.** After stage 2 each of chapters 3–9 kept the digest's inner chapters (`第 1 章：…`, `第 2 章：…`, in chapter 9 also `第 3 章：稳健性`) as a Side card with a Direct chain, so the reader met a "第 1 章" under 第 3 章. Each inner chapter card was deleted `into` its chapter card (15 cards; Links re-pointed) after its figures, estimates and tests moved up as Side branches of the chapter, in the order figure, estimates, limits card. Tests that stood in an inner "识别检验" or "稳健性" chapter moved under the estimate they support: 6\|a\|1\|a under the Republican estimate of (2); 10\|a\|1\|a and 10\|a\|1\|b under the Republican dissent estimate; the SVAR Wald test, the narrative evidence on departures and the comparison with the local projections under the SVAR policy-decision estimate (`4|a|a|11|a`).
- **Merge decision 1 (chapter 3's party split): merged.** The inner chapter `5|a|1` (第 2 章：异质性：任命者党派) and its two estimates restated chapter 4's estimates of the same coefficients $\beta^h$ and $\gamma^h$ of (2), read from the same panels B and C of Figure 2 (source §3.3 reports them once). The estimates were deleted `into` `6|a|b` and `6|a|c` (now `4|a|a|6|b`, `6|c`), the inner chapter into chapter 4. The deleted Republican card said that panel B draws 90% HAC bands; the kept card says the uncertainty is not stated. The kept estimate text was not changed (no estimate is rewritten in this pass); see Open.
- **Merge decision 2 (the two same-titled weak-instrument tests): not merged; another copy merged instead.** The source states the Lewis–Mertens test once (§3.2, footnote 18: 26 projections, 494 tests). The digest bound it to each specification. `4|a|a|4|a` (chapter 2) and `5|a|b|a` (under the baseline estimate) both test specification (1) with the same counts, so `5|a|b|a` was deleted `into` `4|a|a|4|a`. `6|a|1|a` has the same title as `4|a|a|4|a`, but it binds the test to specification (2), which no other card does; it stays as the test of the estimate of (2) (`4|a|a|6|b|a`), like the tests of (3) (`7|c|a`), of the stance specification (`9|b|a`) and of the dissent specification (`10|b|a`). One test card per specification keeps every estimate's evidence chain.
- **§3.5 outcome corrected.** The source (§3.5) says the president-party projections are "局部投影 (3) … 进一步与任命总统所属政党交互", with results compared with Figure 3 column 1 and read as 扩张性/紧缩性货币政策: the outcome is the policy decision, not the stance. The item `1. 因变量：政策立场` of `2|b|d`, `d|a` and `d|b` now reads `1. 因变量：政策决策（1 紧缩，-1 扩张，0 不变）。`; chapter 6's 问题 and 估计对象 say 政策决策, and its cross-book line to `assenza2026louder/4|b|c|3` now says that the stance measure is the outcome of chapter 7, not of this chapter. No formula changed.
- **Chapter cards.** Chapters 3–9 got **直觉** (a lead of 20+ characters), **本章估计** (pointer lines) and **承上启下** with `依赖` lines to their specification and to the chapter they extend (3 ← 2, 4 ← 3, 5 ← 3, 6 ← 5, 7 ← 4, 8 ← 4, 9 ← 3); this anchored all seven. The old 问题/概要/估计对象/设计/主要估计/关联 text stays below, with its formulas. Their titles now start `第 k 章：` and state the finding. Five moved cards of chapter 9 and two others had their inner chapter numbers updated (`这是第 9 章的估计`, `第 9 章的识别检验`, `第 9 章的稳健性检验`; figure 2 `第 3 章与第 4 章的系数序列`; `8|c` `本章的非选举年估计`).
- **Loose 8 → 0.** Each chapter limits card got one plain reference line to its chapter; `4|a|b` got one to the design.
- **直觉** leads prepended to the data, design and baseline specification cards. **结构** rebuilt from the tree (main line and one level under it).

### 8\|b (empirical)

```
8|b|a    数据 → 8|b|a|1 设计 → 8|b|a|2 规格：基准
8|b|a|3  第 1 章：鸽派立场预测扩张性决策，鹰派立场不预测紧缩   (表 3, four estimates)
8|b|a|4  第 2 章：控制绿皮书变量后结果依然成立，并与局部 IV 投影一致 (two tests)
8|b|a|5  第 3 章：分党派后两类任命者的鸽派立场都预示扩张，只有民主党任命者显著 (four estimates)
```

Network: 规格 → 第 1 章 → 第 3 章; 第 2 章 under the specification. The third chapter was numbered 第 4 章 (no chapter 3 existed); facet, title and the references in chapter 2 and 假设 3 now say 第 3 章. Chapter 1's lead (17 characters) was lengthened; **直觉** leads were added to the data, design and specification cards; the limits card `8|b|b` got a reference line to chapters 1 and 3 (loose 1 → 0); **结构** rebuilt. The shared part (chapter 7 of 4\|a) and its Link were left for the import stage.

### Gate

- `check-model.mjs 4|a`: `members 113 {"main":12,"side":99,"off":2}`, `relations {"refers":185,"rests-on":85,"contrasts":1}; loose 0; self_sufficient true`; no closure, forward or unanchored fault (was 7 unanchored, loose 8).
- `check-model.mjs 8|b`: `members 25 {"main":6,"side":17,"shared":1,"off":1}`, `relations {"refers":41,"rests-on":12,"corresponds":1}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` for both (4\|a: 13 blocks, 5 layers; 8\|b: 8 blocks, 5 layers).
- `check-premises.mjs 8|b`: `premises: none used before their card`. `4|a`: 5 hits, all in the setting part and all pointers, as after stage 2: the 变量表 row of $R_t$ and the share measure `4|a|a|b` name the specifications (2) where each written form appears; the policy measure `4|a|a|d` says $Policy_t$ is the dependent variable of (1); the design's comparison 2 and 假设 5 describe what enters (2). None uses an estimate of a later card; the data → design → specification order of the evidence spine is fixed.
- `check-settings.mjs`: both `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` (agent): `{"broken":[]}`. Literature tree 460 → 442 Cards. Formula comparison with the backup: 329 → 329, 0 lost, 0 new.
- Term relink: `{"rounds":1,"ok":true,"book":"riboni2026presidential","cards_refused":0,"links_added":77,"cards_changed":15,"applied":true,"links_repaired":0}`. All checks above, `list_broken_links` and the formula comparison were rerun after it and stay as listed.

### Open

1. Step 3 card text was not redrafted: estimate and test cards keep their digest format, and chapters 3–9 keep their old 问题/概要/主要估计/关联 text below the new sections (it still says `本模型` in places; chapter 9's old **检验与稳健性** list repeats its tests).
2. `4|a|a|6|b` (Republican estimate of (2)) says its uncertainty is not stated; the source draws 90% HAC bands for panel A, and the merged card had claimed them for panel B. Whether to state the bands for panels B and C is left to an estimate-card fix.
3. The weak-instrument test is still written once per specification ((1) in chapter 2, (2), (3), stance, dissents), although the source states it once for all 26 projections; the import stage or a later pass may decide whether the later copies should cite `4|a|a|4|a` instead.
4. Stage 2 open item 3 (several notations for one object in the ladder) stays.
5. 8\|b: the Link to chapter 7 of 4\|a becomes an **引入** line in stage 4.
6. Tree unlocked throughout; no `card_locked` met. Other sessions may write other books on the box; no read-view error occurred.
