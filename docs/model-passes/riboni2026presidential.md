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

## Stage 4: imports and explanations

Date 2026-11-14, live box, under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent, no DeepSeek job: neither model has a Derivation part, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-riboni2026presidential-imports.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Text ran as 2 `edit_card` calls (the two 8\|b setting cards) and 3 scripted `edit_card` calls under `-Allow` (the 8\|b Model Card; the two setting cards again, semicolons split into sentences after R3 advice); `imports.mjs apply` wrote one line. Every scripted write was read back and compared with the intended text (`same`); the Model Card write was guarded by a comparison of its display formulas and `\tag` numbers (unchanged). Only riboni2026presidential Cards were written; no display formula, equation number or estimate changed (the three edited setting cards hold no display formula). No `card_locked`, no `read_view_changed` and no failed batch occurred. Order: 4\|a (the lender), then 8\|b.

Before any edit: `check` exit 0 on both; `plan` 4\|a `nothing to import`; 8\|b `nothing to import`, with advice: 2 tolerated premises (4\|a\|a\|2\|c and 4\|a\|a\|6, behind chapter 7 `4|a|a|9`), the false share `对应 [[4|a|a|9]]` in the Model Card's **关联**, and a possible copy (`8|b|a|b` corresponds to `4|a|a|9`).

### 4\|a (empirical: appointment effect)

- No import line. Its data, instruments, measures and specifications are the paper's own; it has no `假说：` part that names a theory result, and no part rests on another model's part (`check` exit 0, `plan` `nothing to import`). Tolerated 0, false shares 0.
- It now lends `4|a|a|i` (the stance measure, with `4|a|a|i|a` and the closure 4\|a\|a, 4\|a\|a\|b) to 8\|b (`lent`).

### 8\|b (empirical: reaction function)

- The false share pointed at the wrong card. 8\|b takes from 4\|a the **measure** of stance, not chapter 7 (the effect of appointments on stance, where stance is the outcome; in 8\|b stance is a regressor of the same meeting's decision). The measure lives in `4|a|a|i` (测量：目标关键词旁的鹰派与鸽派词对, formula (4)), with its sample card `4|a|a|i|a` (the 17 meetings before March 1976).
- Copy → variant. `8|b|a|b` restated the construction (`本卡的度量与……模型相同`, word pairs, normalization, weighting). Rewritten as a variant: summary `沿用 [[4|a|a|i]] 的词对度量 (4)。本卡只改分组`; **起点** `依赖 [[4|a|a|i]] 的测量` (the whole construction (4)) before `依赖 [[8|b|a]] 的数据`; **内容** states only the change (all governors as one group $H_t, V_t$, bank presidents as another $B^{H}_t, B^{V}_t$; the party split $H^{R}_t \dots V^{D}_t$ as in 4\|a\|a\|i) and that stance is a regressor here and an outcome in chapter 7 of 4\|a. The `对应 [[4|a|a|9]]` Link went with the restatement.
- Data card `8|b|a`: **起点** `原始设定` → `依赖 [[4|a|a]] 的数据：同一批例行 FOMC 会议、会议记录与绿皮书。本卡只列本模型用到的部分。` (same 391 meetings, same Greenbook source, the skill's "own data card rests on it").
- Import: `plan` proposed `整块 [[4|a|a]]` (absorbing 4\|a\|a\|i, since both premises lie in the data tree); `apply` wrote it. It was replaced by hand with `整块 [[riboni2026presidential/4|a|a|i]]：政策立场的词对度量 (4)，目标关键词旁的鹰派与鸽派词对按字数标准化、按陈述字数占比加总到会议层面，连同它所建立的会议数据；本模型按理事与行长分组，构造同一次会议的立场回归元`. Reasons: 8\|b uses the stance measure and the meeting data, not the instruments, the policy coding, the VAR series or the dissent measures of the whole data tree (as assenza2026louder 6\|a took `4|a|a|d`, not `4|a|a`); and the whole tree would bring 4\|a's 变量表 `4|a|a|a` into 8\|b, which the `variable_uncovered` checker bug (velic2025inflation stage 4, Open 1) reads before 8\|b's own table because "4\|" sorts first. The block's closure brings 4\|a\|a and 4\|a\|a\|b, so the data card's premise is covered: members 4\|a\|a\|i, 4\|a\|a\|i\|a, closure 4\|a\|a, 4\|a\|a\|b.
- Model Card **关联**: the sentence `对应 [[4|a|a|9]]：立场度量取自任命对政策立场的局部投影模型。` deleted (replaced by the import line); `参照 [[riboni2026presidential]]` and the cross-book `对应 [[assenza2026louder/4|c|a|a]]` stay.
- Tolerated 2 → 0; false shares 1 → 0; members 25 (1 shared) → 28 (24 own + 4 shared). Symbols (step 6): 8\|b keeps its own 变量表, which defines every symbol it uses; it names no symbol of 4\|a, so no `沿用` line.

### Cross-book blocks

None written. `plan` proposed none, and no part of either model rests on another book's Card. Searched by hand (Model Cards on 绿皮书, FOMC 会议, 理事, 异议, Thornton, 鹰派, 局部投影, 任命): no other book holds the FOMC meeting data, the word-pair stance measure or the appointment instruments. assenza2026louder's entropy measure and communication-rule estimation (4\|b, 4\|c\|a\|a, 5\|a, 6\|a) are comparisons (entropy measures volume, not tone; a rule versus stated stance), kept as the existing `对照`/`对应` Links for the evidence dialogue; furceri2026divide's local projections estimate other effects (comparisons).

### Explanation refresh (step 8)

`explain.mjs plan`: `nothing to explain` on both heads (`parts 0, jobs []`): neither empirical model has a Derivation part (`推导：估计式`). No DeepSeek job ran, so no model, account, requests, time or cost to record; conversation 3b1d187b was not touched. `finish`: `explained 0 part(s); Term links changed 0 Card(s), second plan 0; left 0; refused 0` (exit 0) on both.

### Gate

- `imports.mjs check`: exit 0 on 4\|a and 8\|b (no advice left on 8\|b); `plan`: `nothing to import` on both.
- `get_model`: `self_sufficient: true` on both; tolerated 0 and `false_share` [] on both; 4\|a `lent` to 8\|b (root 4\|a\|a\|i); 8\|b one `整块` import (mode block, members 4\|a\|a\|i, 4\|a\|a\|i\|a; closure 4\|a\|a, 4\|a\|a\|b).
- `explain.mjs finish`: exit 0 on both; a later `plan`: `nothing to explain` on both.
- `check-model.mjs`: 4\|a `members 113 {"main":12,"side":99,"off":2}`, `relations {"refers":185,"rests-on":85,"contrasts":1}; loose 0; self_sufficient true`; 8\|b `members 28 {"shared":4,"main":6,"side":17,"off":1}`, `relations {"refers":43,"rests-on":18}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` on both.
- `check-premises.mjs`: 8\|b `premises: none used before their card`; 4\|a the same 5 pointer hits as at stage 3 (setting part naming (1)/(2)).
- `check-settings.mjs`: both `settings: the data tree and the specification ladder are in place` (no false `variable_uncovered` with the imported block).
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"rounds":1,"cards_changed":1,"links_added":2,"links_repaired":0,"applied":true,"book":"riboni2026presidential","ok":true,"cards_refused":0}`. All checks above and `list_broken_links` were rerun after it and stay as listed.

### Open

1. `8|b|a|4|b` (检验：反应函数结果与图 2 A 面板一致) names Figure 2 panel A only in words; a `对照` Link to 4\|a's figure card would make the comparison navigable. Not added here (a comparison, not an import).
2. furceri2026divide/8\|a has a false share of `riboni2026presidential/4|a|a|6` (a cross-book `对照` in its Model Card); it belongs to furceri's book and was not written.
3. Stage-3 open items 1–4 stay (estimate/test card text not redrafted; panel-B bands of `4|a|a|6|b`; one weak-instrument test per specification; several notations in the ladder).
4. Tree unlocked throughout; no `card_locked` met. Other sessions may write other books on the box; no read-view error occurred.

## Stage 6: evidence dialogue

Date 2026-11-14, live box (owner's request), under `zt-evidence-dialogue` steps 1–5, done in one side session by scripts (`dialogue.mjs survey | apply | check`) and one DeepSeek judging conversation. No subagent ran. Backup: `D:\zt\backups\zt-before-riboni2026presidential-dialogue.sqlite3` (`VACUUM INTO`, 407 MB, `PRAGMA integrity_check` ok; drive D had 225 GB free). The Terms were relinked at the end of stage 4, so the survey ran without a first relink. Both models are empirical; the book has no formal model, so stage 5 did not apply. No Card was written.

### Survey

The models were surveyed one at a time, 8\|b after 4\|a was applied, so the judged record dropped the mirror pairs.

- `survey 4|a`: `{"ours":28,"theirs":1515,"pairs":67}`, 6 messages. Estimand Terms: 1\|a\|a, 理事 (1\|a\|b), 1\|a\|ar, 1\|a\|ay, 1\|a\|aw, 1\|a\|bc, 统一政府 (1\|a\|bd), 1\|a\|bf, 1\|a\|bg, 1\|a\|h and the book root (the `equiv` observation of arrocannarsa2026flight Stage 6 Open 3; it forms no pair). All 67 pairs are within the book: 4\|a estimates against the 8\|b reaction-function coefficients (8\|b\|a\|3\|b–e, 5\|a–d), on the shared Terms 理事 1\|a\|b (16), 1\|a\|n (27), 1\|a\|o (16), 扩张性政策 1\|a\|ab (10), 1\|a\|ac (3).
- `survey 8|b`: `{"ours":8,"theirs":1535,"pairs":52}`, 5 messages, 0 mirrors of the 4\|a pairs. Estimand Terms 1\|a\|b, 1\|a\|bp, 1\|a\|m, 1\|a\|c, 1\|a\|bk, 鸽派 1\|a\|bj, 1\|a\|ah and the book root. All 52 pairs are again within the book: the two chair-stance coefficients 8\|b\|a\|3\|d (鹰派 0.115, s.e. 0.026) and 3\|e (鸽派 −0.497, s.e. 0.133), each against 26 estimates of 4\|a, on 理事 (52), 1\|a\|bj (4), 1\|a\|bk (4). The asymmetry is the script's rule: these two estimates carry no Term of their own beyond the estimand Terms, so they pair on the estimand Terms against the other side's text.
- **The skill does pair 4\|a with 8\|b**, as in assenza2026louder (two empirical models of one book are each other's other side).
- **No cross-book pair, and none was forced.** As in rotunno2026industrial, assenza2026louder and arrocannarsa2026flight, the survey pairs on shared Term addresses, widened only by zt-term-sync Links. Read from a read-only connection to the box: none of the book's 93 Term Cards has a Link to or from another book's Card. The other books' Finding Cards and empirical Model Cards that name FOMC, 联邦公开市场委员会, 任命 or 异议 are 13: assenza2026louder's communication-effort cards (4\|b\|d, 4\|c\|f, 1\|b\|a, 2\|c–f, 4\|b\|c\|6\|a, 6\|b, 6\|c: entropy of statements and the communication rule, not appointee effects or stated stance), and three unrelated matches on 任命/异议 (acemoglu2025nobel/1\|k, hansen2021demand/5\|c\|e, haeckl2026dont/5\|d\|e). velic2025inflation and the other monetary-policy books estimate no appointee share, no member stance and no FOMC decision on stance.

### Judging (DeepSeek)

One conversation for both surveys (the 8\|b messages followed the 4\|a messages, which kept the judge prompt as a cached prefix): `opencode-go/deepseek-v4.1-flash`, effort high (the box's drive model, as in the earlier stage 6 records), OpenCode Go account Primary (pinned), 11 requests (one per message file), about 50 s of model time, context 124k of 1M at the end, no tool call. Prompts were sent as `prompt_files` (`references/judge.zh.md` + `judge-1.md`, then each later file). The Harness ledger moved by 11 opencode-go calls during the job (113,084 uncached input, 842,368 cached, 10,495 output tokens; cache hit about 88%), cost 0 CNY (OpenCode Go subscription; the day line is shared with other sessions, but no other call landed in the window). Conversation 5e210269 deleted, workspace removed. Conversation 3b1d187b was not touched.

All 119 answers came back `same: false`. Checked against the pairs' titles: right. 4\|a estimates the effect of the appointees' share on later decisions, stance, dissents, inflation and output (instrumented, the share is the regressor; in chapter 7 the stance is the outcome); 8\|b estimates the association of the stated stance (the regressor) with the same meeting's decision. The judge named exactly this inversion on the stance pairs (for example 4\|a\|a\|9\|b against 8\|b\|a\|5\|a: "一个的因变量是立场、另一个的自变量是立场"). The descriptive estimates of 4\|a (median vacancy days 177/266, the polarization correlation 0.35) and the SVAR responses set beside a stance coefficient were judged different objects as well.

### Pairs judged and written

| Head | Pairs | Same estimand | Agree / disagree / not comparable | Links written |
|---|---|---|---|---|
| 4\|a | 67 | 0 | 0 / 0 / 0 | 0 |
| 8\|b | 52 | 0 | 0 / 0 / 0 | 0 |

`apply` lines: 4\|a `{"links":0,"written":0,"errors":0}`; 8\|b `{"links":0,"written":0,"errors":0}`. All 119 answered pairs are in the judged record (`~/.zt-evidence-dialogue/judged-default.jsonl`: 67 lines with head 4\|a, 52 with head 8\|b).

### Gate

All lines below are the reruns after the Term relink.

- `dialogue.mjs check`: 4\|a `{"head":"riboni2026presidential/4|a","broken":0,"proposed_again":0,"ok":true}` (exit 0); 8\|b `{"head":"riboni2026presidential/8|b","broken":0,"proposed_again":0,"ok":true}` (exit 0).
- No same-estimand pair, so no **对应** line to check.
- `check-settings.mjs`: 4\|a and 8\|b `settings: the data tree and the specification ladder are in place` (exit 0).
- `check-model.mjs`: 4\|a `members 113 {"main":12,"side":99,"off":2}`, `relations {"refers":185,"rests-on":85,"contrasts":1}; loose 0; self_sufficient true`; 8\|b `members 28 {"shared":4,"main":6,"side":17,"off":1}`, `relations {"refers":43,"rests-on":18}; loose 0; self_sufficient true` (unchanged from stage 4).
- `list_broken_links` (agent): `{"broken":[]}`, before and after the relink.
- Term relink: `{"links_repaired":0,"cards_changed":0,"cards_refused":0,"ok":true,"book":"riboni2026presidential","rounds":0,"applied":false,"links_added":0}`.

### Open

1. No cross-book pair: the book's Terms (理事, 鸽派, 鹰派, 统一政府, …) link to no Term Card of another book. After a zt-term-sync pass on the economics shelf, survey both heads again; the judged record keeps the 119 pairs judged here from being paid for twice. The likeliest neighbour stays assenza2026louder (FOMC text), whose estimates measure communication effort, not stance; the hand-written `对照` Links between the two books (stage 2, policy measure) stay as written.
2. Survey noise: 8\|b\|a\|3\|d and 3\|e carry no Term of what they measure, so they paired with every 4\|a estimate that names 理事 (52 pairs, all `same: false`). A Term link for 行长 / 立场 in those two cards would narrow a later survey; not added here (the stage writes only judged Links).
3. Script observation carried from arrocannarsa2026flight Stage 6 Open 3: the book root appears in `estimand_terms`; it formed no pair. No script fault was found; no skill file changed.
4. Stage-4 Open items 1–3 stay. No `card_locked`, no `read_view_changed` and no failed read or write occurred. Nothing committed (stage brief).

## Stage 7: literature organize

Date 2026-11-14, live box (owner's request), under `zt-literature-organize`, done in one side session by scripts (`outline.mjs`, three structure batches through `batch.mjs`, one `leads.mjs` run, one scripted `edit_card` batch for titles, position lines, the root index and the structure card, and one fix). No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-riboni2026presidential-organize.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Every batch was previewed first (complete true, refusals none, `links_that_will_break` empty) and then committed under its plan hash. Before the text batch a script compared the display and inline formulas and `\tag` numbers of the old and new bodies (0 changes), and every written Card was read back and compared with the intended text (0 differences). The tree went from 442 to 444 Cards (two new summary leads, §1 and §3). No Card was deleted, and no formula, equation number or estimate changed. Only Cards of riboni2026presidential were written; the moves re-pointed Links in other books by themselves (furceri2026divide/8\|a now reads `riboni2026presidential/4|f|a|6`, the same chapter-4 card). No `card_locked`, no `read_view_changed` and no failed batch occurred; the tree was unlocked throughout. Other sessions wrote other books on the box.

### Source divisions

From `D:\zt-digest\riboni2026presidential\book.zh.md`: the title, 摘要, 1. 引言 (1.1 总统与理事会, 1.2 对文献的贡献, `##` 1.3 全文结构), 2. 数据, 3. 计量策略 (`##` 3.1 局部投影, `##` 3.2 外部工具变量, then 3.3 基准结果, 3.4 选举周期, 3.5 党派行为, which the translation marks as top-level `#` but which are subsections of 3), 4. 机制 (4.1 政策立场, also marked `#`; `##` 4.2 政策决策, `##` 4.3 异议行为), 5. 经济结果, 6. 结论. The translation stops before the reference list. The top line takes the abstract and the six numbered sections, then the two lists.

The digest had filed the paper as 10 groups: old 1 = 摘要 + §1.1, with the Term list, the Cited list, the abstract (whose Direct chain held the eight §1.1 notes) and the structure Card under it; old 2 = §1.2; old 3 = §1.3 + §2; old 4 = §3.1–3.2, whose 24 reading notes and events ran on the Direct line of the Model Card 4\|a; old 5–7 = §3.3–3.5 as top Cards; old 8 = §4, with all of §4.1–4.3 on one Direct chain; old 9 = §5; old 10 = §6.

### Final tree (top line and divisions)

```
1   摘要 (old 1|c)                          a 结构卡 (old 1|e)
2   1. 引言 (new summary, demote of old 1)  a 1.1 (old 1; a|f chain = old 1|c|1–8, finding a|f|6|a = old 1|d)
                                            b 1.2 (old 2) · c 1.3 (old 3|c)
3   2. 数据 (old 3, retitled, 1.3 part out)  c chain (old 3|c|1–6; footnote 7 old 3|d now c|1|b)
4   3. 计量策略 (new summary, demote of old 4)
                                            a 3.1 (old 4; a|a chain = old 4|a|1–3)
                                            b 3.2 (old 4|a|4, chain b|1–b|5 = old 4|a|5–9; findings b|1|a, b|2|a = old 4|b, 4|c)
                                            c 3.3 (old 5) · d 3.4 (old 6) · e 3.5 (old 7)
                                            f empirical model, appointment effect (old 4|a)
5   4. 机制 (old 8, rewritten)               a §4 preamble (old 8|d)
                                            b 4.1 (old 8|d|1, chain b|1; finding b|1|b = old 8|e)
                                            c 4.2 (old 8|d|3, chain c|1; findings c|a, c|b = old 8|f, 8|g; c|c empirical model, reaction function = old 8|b)
                                            d 4.3 (old 8|d|5, chain d|1–d|2; findings d|a, d|b, d|1|a = old 8|h, 8|i, 8|j)
6   5. 经济结果 (old 9, retitled)           b VAR chain; findings under figures 6–8 (b|2|a, b|3|a, b|4|a = old 9|c–e)
7   6. 结论 (old 10, retitled)              a chain; findings under figure 9 (a|2|a|a–c = old 10|b–d) and table 3 (a|3|a|a = old 10|e)
8   术语表 (Term list, from 1|a)
9   参考文献 (Cited list, from 1|b)
```

Batches (tail to head): §4 (11 items, 42 renamed), §3 (8 items, 235 renamed), findings + lists + §1 (22 items, 217 renamed). Then `leads.mjs`: two demotes (old 1, old 4) and one rewrite (old 8).

### Crossers and how each was settled

- Old 1 was 摘要 and §1.1 at once and held the Term list, the Cited list and the structure Card. The lists went to the tail (`8`, `9`). The abstract (old 1\|c) became the top Card `1`, with the structure Card under it (`1|a`). Its Direct chain (old 1\|c\|1–8, all `§1.1`) moved under old 1. Old 1 spoke for §1.1 only, so it was demoted: a new §1 summary took its place, and old 1 is the 1.1 head `2|a` (retitled, its abstract sentence and its `§摘要` / `§1. 引言` lines dropped, position and source lines rewritten to §1.1).
- Old 3 was §1.3 and §2: the 1.3 Card (old 3\|c) moved under the §1 summary as `2|c`, and its Direct chain (old 3\|c\|1–6, all `§2`) stayed under old 3, which is now the §2 Card `3` (its 1.3 sentences and `§1.3` line removed, a source line `§2` added).
- Old 4 spoke for §3.1–3.2 only: demoted. A new §3 summary took its place; old 4 is the 3.1 head `4|a` (its §3.2 paragraph and `§3.2` line dropped, since the summary and the 3.2 notes `4|b`, `4|b|4` carry them; position and source lines rewritten to §3.1). Old 5–7 (3.3–3.5, top Cards that are subdivisions) moved under the summary.
- The Model Card 4\|a carried 24 reading notes of §3.1–3.2 (with 9 events) on its Direct line. The chain was cut by source line: old 4\|a\|1–3 (§3.1) under the 3.1 head, old 4\|a\|4–9 (§3.2) as the 3.2 chain. No model member moved: `check-model.mjs` members stay 113 and 28.
- Old 8\|d (§4 preamble) carried all of §4.1–4.3 on its Direct chain: cut by source line into 4.1 (old 8\|d\|1–2), 4.2 (old 8\|d\|3–4) and 4.3 (old 8\|d\|5–7). Old 8 already walked §4.1–4.3 and was rewritten in place.
- Whole units: 4\|a moved once and is now `4|f`, the last Side branch of the §3 summary (its chapters span §2–§5); 8\|b moved once and is now `5|c|c`, under the 4.2 head.

Calls that could go the other way: the abstract as its own top Card rather than under a 前置页 summary (the front matter is the title, which is the root, and the abstract); the 3.2 head is the first-instrument note `4|b` (retitled `3.2 外部工具变量：…`) rather than a new 3.2 summary; 4\|f under the §3 summary rather than under 3.1 where its head is first written; old 8 rewritten rather than demoted (it spoke for all of §4); the §4 preamble `5|a` kept before the subdivisions; the findings moved under the notes that report them (§3.2, §3.3, §3.4, §4, §5, §6) rather than left as Side branches of the division heads.

### Summary leads and other text

- `leads.mjs`: new summaries `2` (1. 引言, demote of old 1, `keep` the §1.1 chain) and `4` (3. 计量策略, demote of old 4, `keep` the §3.1 chain); rewrite `5` (4. 机制). Each has the source's name and main claim as title, paragraphs that walk the subdivisions, `小节索引：`, `在全书中的位置：` and the source line.
- `leads.mjs` turned Term names into Term uses and linked `理事` inside `理事会多数` and `总统任命` inside `每位总统任命 3 至 9 名理事` in the §1 summary; the hand batch rewrote that paragraph (and split three sentences over 40 characters).
- Hand edits (one scripted batch of 17 writes, then one fix): `2|a` (1.1 head, above), `3` (§2, above), `4|a` (3.1 head, above); retitled with the section number: `2|b` 1.2, `2|c` 1.3, `3` 2., `4|b` 3.2, `4|c` 3.3, `4|d` 3.4, `4|e` 3.5, `5|b` 4.1, `5|c` 4.2, `5|d` 4.3, `6` 5., `7` 6. Four pointer Links that named the wrong section were corrected: §5's VAR results in the 1.2 Card, and 第 5 节的经济结果 in the 1.3 Card and in the §2 Card, pointed at old 10 (结论) and now point at `6`; the 结论 Card's `引言提出的问题` now points at the §1 summary `2` instead of the 1.1 head. The fix round added the missing source line `§3.4` to `4|d` (zt-check I3).
- The root index lists the nine top Cards. The structure Card `1|a` got a new **论证** (the top line with its subdivisions) and **模型** (the two empirical models only; its old model list named 4\|a's chapters and its data tree as models, stale since the fold); its **事件** section stays.

### Gate

All lines below are the reruns after the Term relink.

- `check.mjs riboni2026presidential`: top line 1 摘要 (2), 2 1. 引言 (25), 3 2. 数据 (17), 4 3. 计量策略 (168), 5 4. 机制 (43), 6 5. 经济结果 (12), 7 6. 结论 (14), 8 Term list (93), 9 Cited list (69); `no fault`. Against the start snapshot (before the summaries) it reported `442 Cards before, 444 now` (the two new summaries) and the 4\|a unit `138 Cards before, 114 now` (the 24 reading notes moved off the Model Card's Direct line, as above).
- Source-line walk (139 Cards with a source line, whole units excluded): every Card under its own top division, and every second-level branch holds only its own subsection's label (2\|a 1.1, 2\|b 1.2, 2\|c 1.3, 4\|a–e 3.1–3.5, 5\|b–d 4.1–4.3). No crosser left.
- `list_broken_links` (agent): `{"broken":[]}`, before and after the relink.
- `check-model.mjs`: `4|f` `members 113 {"main":12,"side":99,"off":2}`, `relations {"refers":185,"rests-on":85,"contrasts":1}; loose 0; self_sufficient true`; `5|c|c` `members 28 {"shared":4,"main":6,"side":17,"off":1}`, `relations {"refers":43,"rests-on":18}; loose 0; self_sufficient true` (as at stages 4–6).
- `check-network.mjs`: `network: clean` on both.
- `check-settings.mjs`: both `settings: the data tree and the specification ladder are in place`.
- `imports.mjs check`: exit 0 on both (5\|c\|c's `整块` import now names `4|f|a|i`).
- `zt-check --cards` on the root, the structure Card and the 17 written heads: first run `faults: 1` (I3, no source line in `4|d`), fixed once; second run `faults: 0`, exit 0 (advisory L1/L2/I4/R20 lines only).
- Term relink: `{"links_repaired":0,"applied":true,"cards_changed":3,"ok":true,"cards_refused":0,"rounds":1,"links_added":6,"book":"riboni2026presidential"}`; `-PlanOnly` afterwards: `{"ok":true,"links_added":0,"rounds":0,"book":"riboni2026presidential","links_repaired":0,"cards_changed":0,"cards_refused":0,"applied":false}`. No relink bug.

## Result

Final top line: 摘要 · 1. 引言 · 2. 数据 · 3. 计量策略 · 4. 机制 · 5. 经济结果 · 6. 结论 · 术语表 · 参考文献.

Models (final addresses):

- `riboni2026presidential/4|f`, empirical (level 3): the appointment effect (local projections with 2SLS and four external instruments; SVAR-IV for §5), 113 members, data card `4|f|a`, design `4|f|a|1`, specification ladder `4|f|a|2`, chapters `4|f|a|3`–`11` (§3.2–§5); filed as the last Side branch of the §3 summary. It lends the stance measure `4|f|a|i` to 5\|c\|c.
- `riboni2026presidential/5|c|c`, empirical (level 3): the reaction function (stated stance and the same meeting's decision, OLS), 28 members (4 imported: `整块 4|f|a|i` with its closure); filed under the 4.2 head.

Stage 5 (illustrate) did not apply (no formal model). All other stages passed their gates. Open items across the sequence:

1. Restructure step 3 text was not redrafted in 4\|f: estimate and test cards keep their digest format, and chapters 3–9 keep their old 问题/概要/主要估计/关联 text below the new sections (Stage 3).
2. `4|f|a|6|b` (Republican estimate of (2)) says its uncertainty is not stated, although the source draws 90% HAC bands (Stage 3).
3. The weak-instrument test is written once per specification, although the source states it once for all 26 projections (Stage 3).
4. Several notations for one object stay in the specification ladder ($G^{R}_t$ and $R_t$, the letter $D$) (Stage 2).
5. `5|c|c|a|4|b` names Figure 2 panel A only in words; a `对照` Link would make it navigable (Stage 4).
6. No cross-book evidence pair: survey 4\|f and 5\|c\|c again after a zt-term-sync pass on the economics shelf (Stage 6).
7. The 3.5 head `4|e` still says the projections estimate the effect on 政策立场, while the outcome is the policy decision (Stage 3 corrected the specifications and chapter 6, not this reading note) (Stage 7).
8. `leads.mjs` (`linkTerms`) links a shorter Term inside a longer word once the longer Term's first use is taken (`理事` in `理事会多数`); fixed by hand here, the script is unchanged (Stage 7).
9. This pass doc is not committed (the stage brief said no commit).
