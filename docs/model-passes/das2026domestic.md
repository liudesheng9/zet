# das2026domestic: post-digest model passes

Das et al. (2026), *Domestic Debt Restructuring, Central Bank, and Monetary Policy in EMDEs: Lessons from Recent Country Cases*, IMEN1 queue paper 10. The digest run report is `complete: true` (Cited list rebuilt by mode cited-cards). Post-digest sequence (`zt-post-digest`) on the live box at the owner's request (2026-11-21, "Yes, run Das", after the Furceri sequence).

Open owner decision carried from the digest queue: merge Das 8\|a\|12 into 8\|a\|11? This sequence does not decide it.

## Stage 1: fold

Date 2026-11-21, live box. Backup: `D:\zt\backups\zt-before-das2026domestic-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 5 Model Cards (4 concept, 1 empirical), 5 candidate groups; no rests-on Link joined any two (`read` pairs: 5\|b→4\|a kin, 11\|a→3\|a and 4\|a kin, 13\|a→5\|b compare).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 3\|a | §II.B | why a DDR weakens monetary policy (three balance sheets, four channels) | 3\|a | concept; the three-balance-sheet framework (one security G on the sovereign, central-bank and bank balance sheets; the lock makes net saving smaller than the gross haircut; the lock acts through four channels); first card of the model, kept as head |
| 4\|a | §III | same | 3\|a | concept; the same question and primitives continued: the four channels that 3\|a names and says it leaves for later (`四渠道的逐条展开不在本模型内`) |
| 5\|b | §IV | DDR event study of four monetary indicators | 5\|b | empirical; median paths of base money, broad money, the money multiplier and the reserve ratio around 17 DDR cases (t−3 to t+3) |
| 13\|a | 附录 | same | 5\|b | concept; the coding table of the same 17 DDR events (years, crisis type R/E/B, monetary regime): the sample and grouping built for 5\|b, as riboni 3\|a (coding of the policy variable) |
| 11\|a | §V | DDR design: relief against the central bank's viability | 11\|a | concept; own question (how to design a restructuring), borrows only the accounting of 3\|a and two of its channels (`对应` Links) |

The book holds **3 models**: `3|a` (concept), `5|b` (empirical) and `11|a` (concept). The evidence settled every row by the skill's rules.

### Fold (executed 2026-11-21, live box)

Same fold script as furceri2026divide (`fold.mjs`/`drive.mjs`, this session's scratchpad), one run per card: 4\|a went `after 3|a|a|2` (the end of 3\|a's main line), 13\|a went `after 5|b|b` (the top of 5\|b's main line, after the data card). The Direct idea chains 4\|a\|1–12 and 13\|a\|1–5 moved to their section anchors 4 and 13 as Side branches. No folded head had an **引入** line. Both runs returned ok.

| Former head | Now | Facet |
|---|---|---|
| 4\|a | 3\|a\|a\|3 | 第 1 章：四个渠道（§III） |
| 13\|a | 5\|b\|b\|1 | 数据章：17 次重组事件的编码表（附录） |

### Gate

- `model-survey.mjs`: 3 Model Cards, `3|a` (8 parts), `5|b` (38 parts), `11|a` (5 parts), exactly the decided heads.
- `check-model.mjs`: every former head is a part. `3|a` loose 1, `self_sufficient` false (the new chapter has no rests-on line to the setting); `5|b` loose 2, `self_sufficient` false (left to the settings and restructure passes); `11|a` self-sufficient, loose 1.
- Tree 398 Cards before and after; `list_broken_links` empty. Term relink: 0 changes.

### Which models take the later stages

By the sequence skill, a concept model takes the fold only; stages 2–5 run on a larger concept model only on the owner's word. So `3|a` (8 parts) and `11|a` (5 parts) stop here, and their loose parts stay as the fold left them. `5|b` takes stages 2, 3, 4 and 6; stage 5 (illustrate) does not apply, since the book has no formal model. Stage 7 runs on the whole tree.

## Stage 2: settings (strict)

Date 2026-11-21, live box, under `zt-model-settings` (empirical form: the data tree and the specification ladder), done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-das2026domestic-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The tree work ran as one `apply_batch` (preview, `deleted` set compared with the intended one, commit under the plan hash; `links_that_will_break` empty). A first preview that moved the folded coding chapter with `move_card` would have carried the whole main line (the Direct chain after it) into the data tree; it was not committed, and the committed batch moved that one card with `move_span` (`from` = `through`). Text ran as `edit_card` calls under `-Allow` (das2026domestic Cards only); before each write a script checked that every display formula of the old body is in the new one, byte for byte, and every written card was read back (0 differences). No `card_locked`, `scope_refused` or `read_view_changed` error occurred. Tree 398 → 397 Cards (1 deleted `into`, 0 created). Only 5\|b ran; 3\|a and 11\|a were not touched.

### 5\|b (empirical)

Before: the Model Card's first Side successor `5|b|a` was a Literature idea card (footnote 7 on what the four indicators measure), so the data card stood at `5|b|b` (`place` fault); the folded coding table (former 13\|a, stage 1) stood on the main line between the data card and the hypothesis, unanchored; the 变量表 had two unused rows ($i$, $s$); the sample, index-measure and duration cards had no **起点**.

- **Model Card Sides reordered**: data card first (now `5|b|a`), the footnote idea card second (now `5|b|b`, not a member), limits third (`5|b|c`).
- **The coding table went into the data tree.** The folded chapter head left the main line (`move_span`) and became `5|b|a|c` `测量：危机类型与货币政策制度的编码（附录表）`; its 设定 card (每次重组一行，七列编码) was deleted `into` it after its two branches moved out, and its items and **直觉** were written into the measure. The 结论 card became `5|b|a|d` `描述：危机类型与制度的分布`, the 例子 card `5|b|a|c|a` `描述：编码举例`. Old 问题/概要 text of the folded head is gone; its Links stay (book root, `das2026domestic/6`, the 例示 line to 5\|b, gomezcram2026government/4\|b).
- **The ladder.** `规格：按持续时间拆分` moved under the panel-C specification (now `5|b|a|3|b|a`), since it splits the multiplier index of panel C; its `依赖` names `3|b`, its **相对基准的改动** says so. The share specification (`3|d`) now also rests on `3|b` and `3|c`, where the index it counts is defined.
- **Data tree text.** Every sample, measure and description has a one-line summary, **起点** and **内容**. The duration description listed seven multi-year cases and omitted Suriname 2000–2001; it now lists both groups from the appendix table (8 multi-year, 9 one-year; the counts are marked as counted from the table, since the text gives none). The distribution description adds the counts of the crisis flags, counted from the table (R 5, E 5, B 1; 9 events with no flag).
- **变量表** rewritten (17 rows): $i$ and $s$ are named inside the rows of $t$ and $\tau$ (they occur only as indices); the two forms $x^{k}_{i\tau}$, $x^{k}_{i,t+\tau}$ share one row; new rows $\hat Q^{k}_{p}(\tau)$, $p$, $\hat\Delta^{k}(\tau)$, and $t^{\mathrm{end}}$ in the row of $t$.

```
5|b|a      数据：2000–20 年 17 个 DDR 案例的货币指标              原始设定
├ a  变量表 (17 rows)
├ b  样本：17 个案例 (panels 15, 17, 15, 13)
├ c  测量：危机类型与货币政策制度的编码（附录表）   ← 样本 (former 13|a head + its 设定)
│    └ a 描述：编码举例
├ d  描述：危机类型与制度的分布                     ← c
├ e  测量：乘数与准备金比指数
└ f  描述：重组持续时间 (8 multi-year, 9 one-year)  ← b, c
5|b|a|1   假说：四渠道
5|b|a|2   设计 (a–e 假设 1–5)
5|b|a|3   规格：基准 (panel A)
├ a 面板 B;  b 面板 C → b|a 按持续时间拆分;  c 面板 D;  d 低于 DDR 前水平的份额;  e 按货币框架分组
5|b|a|4–6 第 1–3 章 (unchanged; stage 3)
```

### Equations written in (owner rule: every 规格 card)

The digest had written one median formula per specification, introduced by `原书没有写出本规格的估计式。本卡按……补写一个公式（补：书中未写）。` above the section. Each **估计式** section now opens with `原书没有写出本规格的估计式。本卡按 <§IV.A 正文、图 2 注、附录表> 补写：` and the digest's formula stays byte for byte. Eight display formulas were composed and added:

- the interquartile range of panels A–D (`3`, `3|a`, `3|b`, `3|c`), e.g. $\hat Q^{A}_{p}(\tau) = \operatorname{pct}_{p,\; i \in \mathcal{N}_A(\tau)} \, g^{BM}_{i,t+\tau}, \ p \in \{25, 75\}$, the IQR being $[\hat Q^{k}_{25}(\tau), \hat Q^{k}_{75}(\tau)]$ (Figure 2 note: median and 25th–75th percentiles);
- the index relative to the pre-DDR mean (`3|b`, `3|c`): $x^{C}_{i\tau} = 100\, MM_{i,t+\tau} / \big(\tfrac{1}{3}\sum_{s=-3}^{-1} MM_{i,t+s}\big)$, and the same for $RR$ (Figure 2 note: t−3 to t−1 average = 100);
- the duration split (`3|b|a`): $d$ = 多年期 iff $t^{\mathrm{end}} > t$, 一年期 iff $t^{\mathrm{end}} = t$ (text: "spanning more than one calendar year"). A first form with `\begin{cases}` was refused by zt's variable check (`cases` read as a symbol) and rewritten as two `\iff` clauses;
- the regime comparison (`3|e`): $\hat\Delta^{k}(\tau) = \hat m^{k,E}(\tau) - \hat m^{k,M}(\tau)$; the card says that the source reports no test statistic, only "no robust differences".

No number or formula the source states was changed (the source states none for this section).

### Gate

- `check-settings.mjs das2026domestic/5|b --symbols`: `settings: the data tree and the specification ladder are in place` (no fault).
- `check-model.mjs`: 37 members `{"main":7,"side":29,"off":1}`, `relations {"rests-on":19,"illustrates":8,"refers":54,"objects":1}`, loose 1 (the limits card), `self_sufficient` true (was false, with the coding chapter unanchored).
- `check-network.mjs`: 4 `lead` faults (data, hypothesis, design, baseline have no **直觉**); left to the restructure.
- `list_broken_links` (agent): `{"broken":[]}`; tree 397 Cards. **结构** of the Model Card rebuilt from the build order (37 lines).
- Term relink: `{"book":"das2026domestic","cards_changed":8,"cards_refused":0,"applied":true,"rounds":1,"links_repaired":0,"ok":true,"links_added":16}`. Settings check, `check-model` and `list_broken_links` rerun after it: unchanged.

### Open

1. 第 3 章 holds `表 A1：附录事件分类` (`5|b|a|6|a`), which restates the coding table now in the data tree; the restructure decides the merge.
2. The estimate `4|b` (broad money) says `反驳` the hypothesis ("介质的数量总量没有断点"); a flat broad-money median with a falling multiplier is not plainly a refutation of the four-channel prediction. Kept as the digest judged it; the restructure rewords only the sentence.
3. The footnote idea card `5|b|b` stays a Side of the Model Card outside the model.

## Stage 3: restructure

Date 2026-11-21, live box, under `zt-model-restructure` (empirical form: the evidence spine), done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; chapter cards written by hand, estimate cards rebuilt by a script in the role format). Backup: `D:\zt\backups\zt-before-das2026domestic-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). One `apply_batch` (preview, `deleted` set compared, commit under the plan hash; `links_that_will_break` empty); text as 19 `edit_card` calls under `-Allow` (18 cards and the Model Card's **结构**), every card read back (0 differences). Before the write a script checked that no display formula and no image was lost, and listed every Link a card dropped: only the `见 [[第 k 章]]` lines of the ten estimate cards (a card filed under its chapter needs no pointer to it, as in furceri2026divide). No estimate number changed. No `card_locked`, `scope_refused` or `read_view_changed` error occurred. Tree 397 → 396 Cards (1 deleted `into`, 0 created).

### 5\|b (empirical): the evidence spine

```
5|b|a     数据 (data tree as stage 2 left it) → |1 假说：四渠道 → |2 设计 (假设 1–5) → |3 规格：基准 (ladder)
5|b|a|4   第 1 章：四个指标的中位数路径（§IV.A，图 2）   ← 规格      a 面板 A 15.5%; b 面板 B 12.7/12.8%; c 面板 C 98 → 93; d 面板 D −2 至 −5%
5|b|a|5   第 2 章：离散与份额                          ← 第 1 章   a 面板 A 四分位距; b 面板 B 四分位距; c 60% (t+2); d 约四分之三
5|b|a|6   第 3 章：异质性与稳健性                      ← 第 1 章, 编码表   a 货币框架：无稳健差异; b 多年期乘数约 90%
5|b|b     (footnote idea card, outside the model);  5|b|c 局限
```

Network (`check-network.mjs`): 数据 → 假说 → 设计 → 规格 → 第 1 章 → {第 2 章, 第 3 章}; 8 blocks, 6 layers. Three chapters, so no **分部**.

- **Duplicate merged:** `表 A1：附录事件分类` (`5|b|a|6|a`, in 第 3 章) restated the appendix coding table that stage 2 put into the data tree; it was deleted `into` the coding measure `5|b|a|c`, whose item 8 already names the two specifications the table's columns feed (the regime split and the duration split). Inbound Links re-pointed by zt; the two estimates of 第 3 章 moved up to `6|a`, `6|b`.
- **Chapter cards** rewritten in the format **直觉** (a lead of 20+ characters stating the chapter's finding), **本章估计** (one pointer line per estimate, with its number), **承上启下** (`依赖` the baseline specification for 第 1 章, 第 1 章 for 第 2 and 3 章, and the coding table for 第 3 章; plain words for what comes next), 来源; none of the old one-line opening or the old short **直觉** stays.
- **Estimate cards (10) redrafted:** one sentence; **估计** with 估计量 (its specification Link and `出自 §IV.A 正文与图 2 面板 X`), 值 (unchanged, `从原文抄录`), 不确定性 (the source reports no standard error or interval; Figure 2's band is the cross-case IQR, not sampling uncertainty), 显著性 (none reported; 第 3 章: "no robust differences" and "not statistically robust" in the source's words), 样本量 (panels A–D 15, 17, 15, 13 cases; the two splits 8/9 cases from the appendix table, with the note that the source gives no per-group count in panel C); **量级** and **为什么** kept; **依据** (`依赖` the specification, `识别见设计`); **对应** kept; 来源 with section and panel. The `反驳` line of the broad-money estimate kept its relation and target; its sentence was reworded ("介质的数量总量" was a translation slip for broad money).
- **Setting part, text only:** a **直觉** lead was prepended to the data, hypothesis, design and baseline cards (the network's four `lead` faults of stage 2). The loose limits card got one plain reference line to the design and its 假设 5.
- **结构** of the Model Card rebuilt from the build order (36 lines).

### Gate

- `check-model.mjs 5|b`: `members 36 {"main":7,"side":28,"off":1}`, `relations {"rests-on":20,"illustrates":9,"refers":53,"objects":1}; loose 0; self_sufficient true`; no closure, forward or unanchored fault.
- `check-network.mjs`: `network: clean` (was 4 `lead` faults).
- `check-premises.mjs`: `premises: none used before their card`.
- `check-settings.mjs`: `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` (agent): `{"broken":[]}`; tree 396 Cards.
- Term relink: `{"applied":true,"ok":true,"links_added":19,"rounds":1,"links_repaired":0,"cards_changed":12,"cards_refused":0,"book":"das2026domestic"}`. All checks above and `list_broken_links` rerun after it: unchanged.

### Open

1. Stage-2 open item 2 stays: the broad-money estimate `4|b` keeps `反驳` the hypothesis; the owner may prefer `对照`.
2. No figure card for Figure 2: the image sits on the footnote idea card `5|b|b` outside the model; the estimate cards name the panel in 来源.
3. Tree unlocked throughout; no `card_locked` met.

## Stage 4: imports and explanations

Date 2026-11-21, live box, under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent, no DeepSeek job: the explanation plan found nothing, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-das2026domestic-imports.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Text ran as 2 `edit_card` calls under `-Allow` (the Model Card 5\|b and its hypothesis card); both read back (`same`); no display formula changed. 3\|a and 11\|a were not edited: 3\|a only lends its card. No `card_locked`, `scope_refused` or `read_view_changed` error occurred. Tree 396 Cards (unchanged).

Before any edit: `check` exit 0, `plan` `nothing to import`, with one `copy` advice: `5|b|a|1 corresponds to 3|a|a|3 of another model` (the hypothesis named the four-channel chapter with `参照`).

### 5\|b (empirical): the decision on the hypothesis

- **Decision: import `单卡 [[3|a|a|3]]` and let the hypothesis rest on it.** The hypothesis `5|b|a|1` (假说：四渠道) is the prediction of §III that the event study tests: the four-channel chapter of the concept model 3\|a (`3|a|a|3`, 第 1 章：四个渠道, the former 4\|a folded in stage 1). By the import skill's rule for an empirical model, a test imports the prediction it tests `单卡`, and the 假说 part rests on it (`依赖` in place of `参照`). It stays `单卡`: the test takes the prediction, not the accounting framework of 3\|a that the chapter builds on; `plan` proposes no upgrade, and the chapter has no rests-on Link inside 3\|a, so tolerated stays 0.
- Line written (by hand, since `plan` proposes nothing for a `参照` Link) as the new **引入** section before **结构**: `单卡 [[das2026domestic/3|a|a|3]]：第 III 节的四渠道框架；假说 [[das2026domestic/5|b|a|1]] 检验它对中央银行资产负债表渠道与银行信贷渠道的预测（流动性投放、银行中介收缩、可自由支配缓冲下降）`.
- Hypothesis card: `参照 [[3|a|a|3]]：本假说来自它的预测。` → `依赖 [[3|a|a|3]]：本假说检验它的预测（经 **引入**）。`; one sentence added from footnote 7: the four indicators bear only on the central-bank balance-sheet and bank-credit channels, the government-securities and credibility channels are left to the case studies. The hypothesis restates no part of the chapter.
- The estimates that `印证` (and the one that `反驳`) the hypothesis stay comparisons; their **量级** and **为什么** do not contradict the new premise, so none was rewritten.
- Result: members 36 → 37 (1 shared: `3|a|a|3`); tolerated 0; false shares 0; imports one line, mode card, closure []. 3\|a now lends `3|a|a|3` to 5\|b. Symbols (step 6): the imported chapter has no symbol; nothing to settle.

### Cross-book blocks

None written. `plan` proposed none and no part of 5\|b rests on another book's Card. Searched the agent Workspace for Model Cards on domestic debt restructuring, debt restructuring, the money multiplier, reserves and the IMF (2021, Table A-2) sample: only das2026domestic holds them (other hits are Cited, Term or idea Cards of hart1995firms, liu2026managing, sylla2018hamilton, arrocannarsa2026flight, bergin2026exchange, gomezcram2026government, ugolini2017evolution, none a model's premise). The Model Card's cross-book Links stay comparisons for the evidence dialogue: `对照 haeckl2026dont/2|b` and `对应 velic2025inflation/4|b|2` (both lines `跨书关联`; `false_share` is empty).

### Explanation refresh (step 8)

- `explain.mjs plan 5|b`: `nothing to explain` (an empirical model with no `推导：估计式` part); no DeepSeek job.
- `explain.mjs finish 5|b`: `explained 0 part(s); Term links changed 0 Card(s), second plan 0; left 0; refused 0` (exit 0).

### Gate

- `imports.mjs check`: exit 0, no advice left; `plan`: `nothing to import`.
- `get_model`: `self_sufficient: true`; tolerated []; `false_share` []; imports `[{"root":"das2026domestic/3|a|a|3","mode":"card","members":["das2026domestic/3|a|a|3"],"closure":[]}]`; no `closure`, `reach_in`, `forward`, `unanchored`, `import_target` or `import_cycle` fault.
- `explain.mjs finish`: exit 0; a later `plan`: `nothing to explain`.
- `check-model.mjs`: `members 37 {"main":7,"side":28,"shared":1,"off":1}`, `relations {"rests-on":21,"illustrates":9,"refers":53,"objects":1}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean`; `check-premises.mjs`: `premises: none used before their card`; `check-settings.mjs`: `settings: the data tree and the specification ladder are in place`.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"book":"das2026domestic","links_added":6,"links_repaired":0,"cards_changed":2,"cards_refused":0,"ok":true,"applied":true,"rounds":1}`. All checks above and `get_model` rerun after it: unchanged.

### Open

1. 3\|a stays a concept model with `self_sufficient` false (its folded chapter `3|a|a|3` has no rests-on Link to its setting, stage 1); by the sequence skill it takes no further pass without the owner's word. The import does not depend on it.
2. Stage-2/3 open items stay: `4|b` keeps `反驳` the hypothesis; no Figure 2 card inside the model.
3. Stages 5 (illustrate: not applicable, no formal model), 6 (evidence dialogue on 5\|b) and 7 (organize) not run; they wait for the owner's go-ahead.
4. Tree unlocked throughout; no `card_locked` met.

## Stage 6: evidence dialogue

Date 2026-11-21, live box (owner's request), under `zt-evidence-dialogue` steps 0–5, done in one side session by scripts (`dialogue.mjs survey | check`) and one scripted `edit_card` write. No subagent and no DeepSeek job ran (the survey found no pair, so there was nothing to judge): no DeepSeek cost, nothing to delete or remove. Backup: `D:\zt\backups\zt-before-das2026domestic-dialogue.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). The Term relink at the end of stage 4 left no Card to change since, so the survey ran without a first relink (step 0). Only 5\|b is empirical; 3\|a and 11\|a are concept models and have no dialogue.

### Survey

- `survey 5|b` (out folder `D:\zt-dialogue\das2026domestic`): `{"head":"das2026domestic/5|b","ours":10,"theirs":1599,"pairs":0,"messages":[]}` (about 59 s). The economics shelf has had no zt-term-sync, so the estimand Terms (基础货币 `1|a|s`, 广义货币 `1|a|t`, 货币乘数 `1|a|r`, 国内债务重组 `1|a|a`) link no Term Card of another book.
- **Hand search** (`search_cards`, Finding Cards and Model Cards: 货币乘数, 基础货币, 广义货币, 准备金, 流动性, 货币供应, 中央银行, 信贷, 债务): no other book estimates base money, broad money, the money multiplier, the reserve ratio or central-bank liquidity around a debt or financial crisis. Every hit on the four indicators is a das2026domestic Card (the §IV.A findings `5|c`–`5|h`, the Jamaica case `6|g`, `7|h`). The nearest Cards elsewhere are not the same estimand: arrocannarsa2026flight `6|d|d` (the central bank expands supply by 4.7% of the stock) and `10|g|c`, `10|g|f` are simulations of the formal model 3\|e, not estimates; velic2025inflation `5|d|n`–`p` estimate central-bank independence against inflation; bergin2026exchange 4\|d estimates reserve accumulation against sector productivity; assenza2026louder estimates central-bank communication; gomezcram2026government, ozhan2026monetary and furceri2026divide hold no Finding Card or estimate on monetary aggregates; atkinson2015lectures holds only formal debt models. sylla2018hamilton `20|a|6` already links `6|g` as `对照` (`跨书关联`, a historical idea Card, no estimate). The Model Card's cross-book Links (`对照 haeckl2026dont/2|b`, `对应 velic2025inflation/4|b|2`) compare designs, not estimates, and stay as written.
- The book's own Finding Cards that state 5\|b's estimates are of the same book, so the survey does not pair them.

### Pairs judged and written

| Head | Pairs | Same estimand | Agree / disagree / not comparable | Links written |
|---|---|---|---|---|
| 5\|b | 0 | 0 | 0 / 0 / 0 | 0 |

No `apply` ran.

### The stage-2/3/4 open item: `5|b|a|4|b` against the hypothesis

The broad-money estimate `5|b|a|4|b` carried `反驳 [[5|b|a|1]]` (the four-channel hypothesis). Read against §IV.A and footnote 7 of the source: footnote 7 names broad-money growth (panel B) and the multiplier (panel C) together as the summary of the bank-credit channel; the text reports panel B only as "no clear break", with the change in its dispersion; and the closing paragraph reads Figure 2 as a whole as "DDRs are typically accompanied by central-bank liquidity provision and some contraction of bank intermediation and of discretionary buffers", i.e. as support for the framework. The source never presents the flat broad-money median as evidence against the hypothesis; with base money rising, a flat broad-money median is the other side of the falling multiplier. It is not plainly 印证 either (on its own it shows no contraction). The word the source supports is therefore **对照**. The line now reads `对照 [[das2026domestic/5|b|a|1]]：广义货币增长没有断点，原文没有把它读作反对四渠道预测的证据；银行中介的收缩由乘数（面板 C）显示，基础货币增长加快而广义货币增长不变，正是乘数下降的另一面。` Written by script (`edit_card`, read back `same`); a check confirmed that no number outside the line changed. The relation in `check-model` turned from `objects` 1 to `contrasts` 1.

### Gate

- `dialogue.mjs check`: `{"head":"das2026domestic/5|b","broken":0,"proposed_again":0,"ok":true}` (the second survey inside the check proposes nothing). No pair, so no written pair to check.
- `check-model.mjs 5|b`: `members 37 {"main":7,"side":28,"shared":1,"off":1}`, `relations {"rests-on":21,"illustrates":9,"refers":53,"contrasts":1}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean`; `check-premises.mjs`: `premises: none used before their card`; `check-settings.mjs --symbols`: `settings: the data tree and the specification ladder are in place`; `imports.mjs check`: `takes what it uses from other models by import lines`.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"applied":true,"links_repaired":0,"cards_changed":1,"cards_refused":0,"links_added":1,"ok":true,"rounds":1}` (the new sentence). The checks above rerun after it: unchanged.

### Open

1. No cross-book pair: survey 5\|b again after a zt-term-sync pass on the economics shelf (at its stage-7 address).
2. No `card_locked`, `scope_refused` or `read_view_changed` error occurred.
