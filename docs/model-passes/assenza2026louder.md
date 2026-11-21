# assenza2026louder: post-digest model passes

Assenza et al. (2026), *Louder than Rates* (NBER conference paper f249548), IMEN1 queue paper 3. Digest run wf_c8b337b2-d89 (2026-11-13, OpenCode Go), run report `complete: true`, 0 gaps. Post-digest sequence (`zt-post-digest`) on the live box at the owner's request, run by a side session of the IMEN1 queue (2026-11-13/14) while the queue digested other books on the same box. Subagents run at effort medium (owner).

## Stage 1: fold

Date 2026-11-14, live box. Backup: `D:\zt\backups\zt-before-assenza2026louder-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 10 Model Cards (6 formal, 3 empirical, 1 concept), 10 candidate groups (no rests-on Link joined two Model Cards; 10\|a imported nine parts of 9\|a).

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 7\|a | §3.1 | CRANK | 7\|a | formal; the household and information block of the paper's one theory model (incomplete-information NK with a communication rule); first formal card of §3, kept as head |
| 2\|b | 引言 | CRANK | 7\|a | concept; the introduction's verbal version of the same model (same agents, shocks, signal, two rules, the three results); becomes the intuition chapter |
| 8\|a | §3.1.2 | CRANK | 7\|a | formal; the firms and Rotemberg pricing block of the same model; same agents and timing |
| 8\|b | §3.1.2 | CRANK | 7\|a | formal; the monetary authority block (Taylor rule (23), communication rule (24)) of the same model |
| 9\|a | §3.2 | CRANK | 7\|a | formal; derives the same model's dynamic IS equation from the household's Euler equation under the signal of 7\|a |
| 10\|a | §3.3 | CRANK | 7\|a | formal; solves the same model's four-equation system (determinacy, loadings, IRFs, ZLB); imported 9\|a's IS equation |
| 5\|a | §2.2 | Fed communication reaction function | 5\|a | formal; the estimable partial-adjustment rule (3)–(4) for the change in statement entropy; its own question; estimated by 6\|a (a theory and its estimation are never folded; as velic2025inflation 3\|a / 8\|a) |
| 4\|a | §2.1 | entropy measure of communication effort | 4\|a | empirical; the measurement and its descriptive facts (Table 1, Figure 1) built for the whole paper: the series feeds both 6\|a and 6\|a\|28, so it cannot belong to one estimation (as velic2025inflation 7\|a) |
| 6\|a | §2.2 | US rule estimation (iterated GMM) | 6\|a | empirical; estimates 5\|a on US data 1994–2022 |
| 6\|a\|28 | §2.3 | cross-country rule estimation | 6\|a\|28 | empirical; the same question on a different data set (five other central banks, own extensions (9)): an empirical model of its own |

Five models. The evidence settled every row (no owner question).

### Fold (executed 2026-11-14, live box)

Method: the per-card fold script of the bergin2026exchange pass (`foldb.mjs`; copy `fold.mjs` in this session's scratchpad), one Card per run, previewed, then committed: strip **结构**/**引入**, mask every Link to a Model Card or part, park the parts beside the section anchor, retype (facet `第 N 章：…`, emphasis core, level cleared), move the head `alone` into 7\|a's main line, return the parts, restore the body with the rename map. 10\|a's Idea-note Direct chain (10\|a\|1–24) went to section 10 (now `10|b` …). 10\|a's **引入** line (`整块` 9\|a\|a\|8, the IS equation (30)) now names a part of the head and was not appended; 7\|a has no **引入** section.

Incident: the 9\|a run's final batch (move the head into the main line, return its parts) failed at commit with no reply (the queue session wrote the box at the same moment); the retype and the park had gone through and the restore wrote the unmasked body. The same batch was rerun at once and applied. The script now retries a batch up to four times.

| Former head | Now | Facet |
|---|---|---|
| 2\|b | 7\|a\|a\|1 | 第 1 章：文字版本与直觉（引言）; directly after the setting card |
| 8\|a | 7\|a\|a\|13 | 第 2 章：厂商与定价（§3.1.2） |
| 8\|b | 7\|a\|a\|14 | 第 3 章：货币当局的两条规则（§3.1.2） |
| 9\|a | 7\|a\|a\|15 | 第 4 章：不完全信息下的动态 IS 方程（§3.2） |
| 10\|a | 7\|a\|a\|16 | 第 5 章：均衡动态（§3.3） |

7\|a's own household and information block stays the setting's Direct chain (7\|a\|a\|2–12); the restructure makes it a chapter.

Gate:

- `model-survey.mjs assenza2026louder`: 5 Model Cards, exactly the decided heads: 7\|a (formal, 83 parts), 5\|a (formal, 6), 4\|a (empirical, 31), 6\|a (empirical, 52), 6\|a\|28 (empirical, 32).
- `check-model.mjs 7|a`: 83 members (17 main, 66 side); every former head a part; six `forward` faults (chapter 7\|a\|a\|14 names its own parts with `依赖` lines) and `unanchored` on the chapter cards 7\|a\|a\|1, 13, 14, 15: left to the restructure. 5\|a, 4\|a, 6\|a, 6\|a\|28: no fault, `self_sufficient` true (loose 0, 2, 2, 0).
- `list_broken_links` (agent): empty.
- Term relink: `{"rounds":1,"ok":true,"cards_changed":3,"links_added":3,"cards_refused":0,"links_repaired":0}`.

## Stage 2: settings (strict)

Date 2026-11-14, live box, under `zt-model-settings`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-assenza2026louder-settings.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Moves and the one delete ran as `apply_batch` tree batches (preview, then commit under the plan hash; `links_that_will_break` empty in every preview). Text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the `\tag` numbers of the old and new bodies; no display formula and no equation number changed. No `card_locked` error and no failed batch occurred. The Literature tree went from 721 Cards to 722: two notation cards were created (5\|a, 7\|a) and one restating assumption card was deleted (7\|a).

### 4\|a (empirical)

- The five cards of the data tree (sample 4\|a\|a\|b, measures c and d, descriptions e and f) now have a one-line summary, a **起点** section with their `依赖` lines and the former `见` pointer, and a **内容** section.
- The 变量表 rows $\mathcal{R}$ and $\mathcal{U}$ were reported as unused because the specifications write them only in subscripts. The sample card now defines both sets in prose (`常规声明的集合记为 $\mathcal{R}$ …`), and the rows name the sample card as their first card. The measure card 4\|a\|a\|c now defines $\mathcal{X}_{s}$ at its first use. No row was dropped.

### 5\|a (formal)

The setting part was built from the former setting card and the cards on its main line:

```
5|a|a   设定：FOMC 委员会、熵的变化与信息集            原始设定
├ a  符号表与读法 (new; blocks: 委员会与信息集, 假设, 目标值方程 (4), 术语)
├ b  假设：自回归成分、前瞻成分、长期一致与对数差分 (from 5|a|a|1)   ← 5|a|a
├ c  设定：部分调整 (3) (from 5|a|a|2, formerly facet 反应函数)    ← 5|a|a, b
└ d  设定：目标值 (4) (from 5|a|a|3, formerly facet 反应函数)      ← 5|a|a, b
```

- Equations (3) and (4) are the whole estimable rule, so they became settings, as the skill requires for an estimation framework. The steady-state derivation is now the only chapter card (`5|a|a|1`), and the figure of the adjustment path (former 5\|a\|a\|2\|a) moved under it as `5|a|a|1|a`.
- The symbol and term tables of the setting card went into the notation card; rows for the index $t$ and the expectation operator $\mathbb{E}$ were added. The sentence on the omitted communication shock moved to the **说明** of the assumption card.

### 6\|a (empirical)

- The measures 6\|a\|a\|b–e now have **起点** and **内容** sections.
- The eight variants 6\|a\|a\|3\|a–g (with 3\|f\|a) open with **相对基准的改动** instead of the sentence `相对基准只改一处：…`. Each 估计式 section now opens with `原书没有写出本规格的估计式。…本卡按…补写：`, except 3\|f\|a, which states a coefficient difference and keeps its own opening.

### 6\|a\|28 (empirical)

- The measure 6\|a\|28\|a\|b and the description 6\|a\|28\|a\|c now have **起点** and **内容** sections.
- The 变量表 row `$FFR_{t}$` was read as the letters $F$ and $R$; it is now written `$\mathrm{FFR}_{t}$`, as the specifications write it.

### 7\|a (formal)

Every setting left its chapter. The derivations that were filed on a setting's Direct chain moved up into the chapter's Side slot, so each chapter now opens on its own derivations (`7|a|a|1|a`, `11|a`, `12|a`, `13|a`). The information assumption and the budget constraint left the main line, so the chapters renumbered: 第 2 章 is now 7\|a\|a\|11, 第 3 章 12, 第 4 章 13, 第 5 章 14.

```
7|a|a   设定：代表性家庭、偏好与冲击的持久—暂时分解        原始设定
├ a  符号表与读法 (new; blocks: 下标, 家庭偏好与冲击, 信息结构, 预算约束与价格, 厂商与定价, 货币当局与两条规则, 推导中的辅助符号, 术语)
├ b  设定：概述（引言） (from 7|a|a|1|a)                         原始设定
├ c  假设：基本面可观测但不可分解，公共信号带噪声 (from 7|a|a|2)    ← 7|a|a
├ d  设定：预算约束 (21) (from 7|a|a|3, formerly facet 约束)      ← 7|a|a, c
├ e  设定：厂商 (from 8|a's setting, 7|a|a|13|a)                  ← 7|a|a
│    e|a 假设：二次价格调整成本 (from 7|a|a|13|a|4)               ← e
├ f  设定：货币当局 (from 8|b's setting, 7|a|a|14|a)              ← 7|a|a, c
│    f|a 假设：两条规则的信息集不同 (from 7|a|a|14|a|1)            ← f
│        f|a|a 设定：利率规则 (23) (from 7|a|a|14|a|2, formerly facet 规则)  ← f, f|a
│        f|a|b 设定：沟通规则 (24) (from 7|a|a|14|a|3, formerly facet 规则)  ← f, f|a
├ g  设定：欧拉方程的信息集 (from 9|a's setting, 7|a|a|15|a)       ← 7|a|a, c, d
│    g|a 假设：不完全信息只作用于条件矩 (from 7|a|a|15|a|1)        ← g
└ h  设定：变体：零利率下限 (from 7|a|a|16|a|21)                   变体，与 f|a|a 并列 ← f|a|a
```

- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**. The four symbol tables and four term tables of the folded setting cards went into the notation card, which covers every symbol of every setting and the auxiliary symbols of the derivations (including $g(x)$, $1-\Gamma_i$, $z$, $\Upsilon$, $\mathcal M$ and $a$, which chapter cards introduce with 记). Each row names the card where a reader meets the symbol first.
- Retyped, not moved: the former setting card of chapter 5 (now `7|a|a|14|a`) states the log-linear four-equation system (30)–(33), which chapters 3 and 4 derive; it rests on Proposition 2. It now carries the facet `均衡系统` and stays at the head of chapter 5. The definitions of the risk component (`7|a|a|13|a`) and of the auxiliary parameters $\Delta$, $\Delta_i$ (`7|a|a|14|a|9`) define derived values and stay in their chapters, as in the bergin pass.
- Deleted `into` 7\|a\|a\|c: the chapter-5 assumption 7\|a\|a\|16\|a\|1 restated the shock decomposition, the observation assumption and the sign of the communication rule. Its one new statement, the total revision $\Gamma_i=1-(1-G_i)(1-k_i)$, now stands in the **说明** of `c`. Its Links were re-pointed to `c`.
- Statements moved and pointers: the Rotemberg assumption `e|a` no longer rests on the demand and marginal-cost derivations of chapter 2; its **说明** points to them. The zero-lower-bound variant rests on the rule (23) instead of the chapter-5 system, which its **说明** names by a pointer. The information-set setting `g` now points to the derivation of the posterior (18) instead of using it. The rule cards dropped the sentence that restated the information set of `f|a`; `f|a` dropped the steady-state normalization that `f` states.
- Definitions added at first use: $c_t$, $n_t$ and $\widetilde{\mathbb{E}}_0$ in the setting card; $P_t$ in `d`; $n_t(j)$ in `e`; $p_t(j)$, $p_t$, $mc_t(j)$ in `e|a`; $\widetilde{\boldsymbol{\tau}}_{t-1}$ in `c`.

The **结构** sections of the Model Cards 5\|a and 7\|a were rebuilt from the build order.

### Gate

- `check-settings.mjs assenza2026louder/4|a`: `settings: the data tree and the specification ladder are in place`.
- `check-settings.mjs assenza2026louder/5|a`: `settings: all in the setting part, as a dependency tree`.
- `check-settings.mjs assenza2026louder/6|a`: `settings: the data tree and the specification ladder are in place`.
- `check-settings.mjs assenza2026louder/6|a|28`: `settings: the data tree and the specification ladder are in place`.
- `check-settings.mjs assenza2026louder/7|a`: `settings: all in the setting part, as a dependency tree`.
- `check-model.mjs 5|a`: 7 members, loose 0, `self_sufficient` true.
- `check-model.mjs 7|a`: 83 members (15 main, 68 side), no closure fault; two `forward` faults (chapter 3, 7\|a\|a\|12, names its own result 12\|a\|2 and correspondence 12\|b with `依赖` lines; stage 1 had six) and `unanchored` on chapters 1, 2, 4 and 5 (7\|a\|a\|1, 11, 13, 14), so `self_sufficient` stays false; loose 2 (7\|a\|a\|1, 13\|b). These are left to the restructure, as after stage 1.
- `list_broken_links` (agent): empty, before and after the relink.
- Term relink: `{"ok":true,"book":"assenza2026louder","cards_changed":18,"links_repaired":0,"cards_refused":0,"applied":true,"links_added":57,"rounds":1}`. The five checks were rerun after it and stay clean.

### Open

1. 7\|a: chapters 1, 2, 4 and 5 are unanchored, and chapter 3 has two forward faults; the household-block derivations (7\|a\|a\|2–10) still stand on the main line between chapter 1 and chapter 2. The restructure builds the spine.
2. 7\|a\|a\|14\|a (`均衡系统`) collects results of chapters 3 and 4 and carries a **直觉** section; the restructure decides whether it stays the head of chapter 5 or becomes a result card.
3. `check-premises.mjs` lists pointers from settings to later equations ((3), (4) in 5\|a; (18), (20), (32), (36), (46) in 7\|a). All are pointers, not uses.
4. Closed in the addendum below: the IV/GMM variants 6\|a\|a\|3\|a–g and 6\|a\|a\|3\|f, and 6\|a\|28\|a\|3\|a–c, now state their first-stage (instrument) equations as 补写 blocks; no existing formula changed.
5. The notation card of 7\|a names the setting card as the first card of $n_t(j)$, because the check matches $n_t$ there; the symbol is defined in `e`.

### Addendum: first-stage equations (2026-11-14)

The owner's rule of 2026-11-12 (formal estimating equations) requires every IV or GMM specification to state both stages. A second backup was taken first: `D:\zt\backups\zt-before-assenza2026louder-settings2.sqlite3` (integrity ok). Each card below received one new paragraph before **各项**, opening `原书没有写出本规格的第一阶段（工具变量）方程。本卡按……补写：`, followed by the composed first-stage equation in the source's notation. A script confirmed that every existing display formula stayed byte for byte and that each card gained exactly one display formula.

- 6\|a: the variants 6\|a\|a\|3\|a (inflation measure), 3\|b (output-gap measure), 3\|c (horizons $k,q$), 3\|d (Tealbook forecasts, two lags), 3\|e (external instruments), 3\|f (log length) and 3\|g (post-2008 sample) now state $\pi=\gamma_{\pi}'z+u_{\pi}$ and $\tilde y=\gamma_{y}'z+u_{y}$ in the form of their own second stage.
- 6\|a\|28: 6\|a\|28\|a\|3\|a and 3\|b now also state the inflation and output-gap first stages (they stated only the exchange-rate or federal-funds-rate first stage); 3\|c now states the federal-funds-rate first stage.
- Not changed: the two baselines (6\|a\|a\|3, 6\|a\|28\|a\|3) already state both stages; 6\|a\|a\|3\|f\|a is a coefficient difference with no regression; the 4\|a specifications are descriptive and each already has its equation; 5\|a has no instruments.

Gate after the addendum: `check-settings.mjs` reports `settings: the data tree and the specification ladder are in place` for 4\|a, 6\|a and 6\|a\|28; `list_broken_links` (agent) empty; term relink `{"links_repaired":0,"rounds":1,"book":"assenza2026louder","ok":true,"applied":true,"links_added":2,"cards_refused":0,"cards_changed":2}`.

## Stage 3: restructure

Date 2026-11-14, live box, under `zt-model-restructure`, done in one side session by scripts (no subagent, no DeepSeek job, so no DeepSeek cost; the chapter leads were written by hand). Backup: `D:\zt\backups\zt-before-assenza2026louder-restructure.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Moves ran as `apply_batch` tree batches (preview, then commit under the plan hash; `links_that_will_break` empty in every preview); text ran as `edit_card` calls under `-Allow`. Before each text write a script compared the display formulas and the equation numbers of the old and new bodies; no display formula and no equation number changed (the Model Card 7\|a lost its equation numbers only with its old **结构** and summary, which were rebuilt). No `card_locked` error, no `read_view_changed` and no failed batch occurred. The Literature tree went from 722 to 725 Cards (three new chapter cards: 5\|a\|a\|1, 7\|a\|a\|2, 7\|a\|a\|7). The setting parts kept their structure; the only edits there were two chapter numbers in text and the first-meeting column of seven notation rows (below).

### 4\|a (empirical): the evidence spine

Main line: 数据 `4|a|a` → 设计 `|1` → 规格：基准 `|2` → 第 1 章 声明熵的水平 `|3` (Table 1) → 第 2 章 季度变化序列 `|4` (Figure 1) → 第 3 章 识别检验 `|5`. The paper has no hypothesis here (measurement and description). Network: chapters 1–3 under the baseline specification.

- The chapters were numbered 0–2; they are now 1–3 (facets, titles and the chapter numbers in the **承上启下** and `依赖` lines).
- The two limits cards left the Model Card's side and went under what they limit: 熵只度量沟通供给 (former 4\|a\|b) → `4|a|a|5|c` under chapter 3; 非常规声明只有 14 份 (former 4\|a\|c) → `4|a|a|3|d|a` under the estimate of the mean for non-regular statements. Each got one plain reference line to that card, so loose fell from 2 to 0.
- **直觉** leads added to the data, design and specification cards (one sentence each, prepended).

### 5\|a (formal): the intuition spine

```
5|a|a  设定 (setting part as the settings pass left it)
5|a|a|1  第 1 章：熵的变化按部分调整趋向目标值，长期停在合意的稳态值 (new)  ← 5|a|a|c, 5|a|a|d
    a 推导：稳态 (former 5|a|a|1)
        a 图示：部分调整路径
```

The steady-state derivation no longer stands alone on the main line: a new chapter card (facet `第 1 章：调整与稳态（§2.2）`, **直觉**/**本章结果**/**承上启下**, `依赖` the settings (3) and (4)) holds it as its Side branch. The chapter has no `结果：` card (advice only): the derivation is the whole result.

### 6\|a (empirical): the evidence spine

Main line unchanged: 数据 → 假说 `|1` → 设计 `|2` → 规格：基准 `|3` → 第 1 章 主要估计 `|4` → 第 2 章 识别检验 `|5` → 第 3 章 稳健性 `|6` → 第 4 章 异质性与机制 `|7`. Network: 1 → {3, 4}; 2 under the specification.

- Limits cards moved under what they limit: 归一化 (former 6\|a\|b) → `6|a|a|4|c|a` under the estimate of the inflation anchor; 需求侧外部工具变量 (former 6\|a\|c) → `6|a|a|6|k|a` under the IV2 estimate. One plain reference line each; loose 2 → 0.
- **直觉** leads added to the data, hypothesis, design and specification cards. Chapter 3's lead (18 characters) was lengthened to state what was re-estimated.

### 6\|a\|28 (empirical): the evidence spine

Main line unchanged: 数据 → 假说 `|1` → 设计 `|2` → 规格：基准 `|3` → 第 1 章 基准规则的跨国估计 `|4` → 第 2 章 识别检验 `|5` → 第 3 章 汇率与美国利率扩展 `|6`. Network: 1 → {2, 3}.

- The limits card 跨国系数不可直接横比 (former 6\|a\|28\|b) → `6|a|28|a|4|g` under chapter 1, with one reference line.
- **直觉** added to the data, hypothesis, design and specification cards; the three chapter cards had their opening sentence but no **直觉** heading, which was added.

### 7\|a (formal, CRANK): the intuition spine

```
7|a|a  设定 (setting part as the settings pass left it)
7|a|a|1  第 1 章：文字版本与直觉（引言）                 ← 设定, 7|a|a|b
    a 机制 → a|1 结论 → a|1|a 例子 (unchanged)
7|a|a|2  第 2 章：家庭的信号提取（§3.1.1） (new)         ← 7|a|a|c
    a 结果：沟通质量的充分统计量 K   (proof a|a 先读基本面 (17) → a|a|1 再读信号 (18))
    b 结果：引理 1 误差衰减更快      (proof b|a 逐冲击 (19) → 平稳先验方差 → 沟通前新息 → 误差分解 (20) → 引理一;  b|b 图示)
7|a|a|3  第 3 章：厂商与定价（§3.1.2）                    ← 7|a|a|e
    a 结果：静态定价条件 (22)        (proof a|a 需求 → 价格指数 → 边际成本 → 一阶条件 → 对称均衡;  a|b 图示)
7|a|a|4  第 4 章：货币当局的两条规则（§3.1.3）            ← 7|a|a|f|a|a, f|a|b
    a 结果：两条规则共用通胀目标     (proof a|a 沟通规则取对数 → 利率规则取对数)
    b 对应：(24) 与第 2 节的符号约定
7|a|a|5  第 5 章：不完全信息下的动态 IS 方程（§3.2）       ← 第 2 章, 7|a|a|g
    a 定义：风险成分
    b 结果：命题 1 风险即沟通 (27)(28)  (proof b|a 改写欧拉方程 → 式 (26) → 一阶近似)
    c 结果：命题 2 动态 IS 方程 (29)    (proof c|a 代入)
    d 结果：均值扭曲的两个成分          (proof d|a 分解)
    e 结果：风险渠道刺激需求
    f 局限：沟通有效性的来源与变体
7|a|a|6  第 6 章：均衡动态（§3.3）                         ← 第 5 章, 第 3 章, 第 4 章
    a 均衡系统 (30)–(33)
    b 结果：利率与沟通可替代
    c 结果：命题 3 贴现与复利          (proof c|a 改写沟通规则 → 简约式 (34) → Θ 的范围;  c|b 说明)
    d 结果：命题 4 局部确定 (35)(36)   (proof d|a 纯前瞻产出方程 → 局部确定性条件;  d|b 例子)
    e 结果：命题 5 产出分解 (37)–(39)  (proof e|a 定义 Δ, Δ_i → 闭式解 → 载荷)
    f 结果：均值扭曲的消退 (43)        (proof f|a 脉冲响应 → 当期均值扭曲比 (44))
    g 结果：推论 1 响应的符号
    h 结果：推论 2 逆周期沟通          (h|a 例子)
    i 结果：推论 3 响应的形状          (proof i|a 极值时点 (45))
7|a|a|7  第 7 章：零利率下限（§3.3） (new)                 ← 第 6 章, 7|a|a|h
    a 结果：零下限处的确定性 (47)      (proof a|a 零下限的确定性条件 (46) → 有效乘数的走廊;  a|b 例子)
    b 结果：双重使命必要不充分 (48)    (proof b|a 产出敏感性的上界)
```

- The household and information derivations (former 7\|a\|a\|2–10) left the main line. A new chapter card `7|a|a|2` holds them as two results with their proof chains; it took the Model Card's old household-block 问题 and 概要 (with `本模型` → `本章` and the symbols written out in words). The later chapters renumbered 2–5 → 3–6 (facets and titles `第 k 章：…`; chapter 4's facet now names §3.1.3, the section its source line gives).
- The §3.3 zero-lower-bound results (former 14\|a\|20–24) left chapter 5 for a new chapter `7|a|a|7`: chapter 6 already held 30 Cards, and the zero lower bound has its own setting variant `7|a|a|h`.
- In every chapter the flat derivation chain was cut into results with their proof chains as Side branches (one batch of 42 tree items, 65 Cards renamed). Result facets read `结果：…` (18 Cards; the former `结果`, `命题`, `命题 1–5`, `引理`, `推论 1–3` facets; titles unchanged).
- Decision on `均衡系统` (now `7|a|a|6|a`): it stays the first card of chapter 6, as the system every result of that chapter starts from, and keeps its facet and **直觉**. It no longer carries the chapter's chain: the results are Side branches of the chapter card beside it. It is no result of chapter 6 (it collects (30) from chapter 5, (31) from chapter 3 and (32)(33) from chapter 4), so it got no `结果：` facet.
- Every chapter card got **直觉** (a lead of 20+ characters), **本章结果** (pointers `结果：[[…]]`) and **承上启下** with `依赖` lines to the earlier chapter or setting it builds on; the old text follows (`本模型`/`那一模型` → `本章`/`那一章` in the chapter cards). This anchored chapters 1–5 and 7. The two forward faults of chapter 4 (the chapter rested by `依赖` on its own result 4\|a and its correspondence 4\|b) were lines of the chapter's table of contents, not premises: the chapter card states the rules and does not use their log-linear form. They are now the **本章结果** lines `结果：[[4|a]]` and `旁注：[[4|b]]`; the chapter rests on the two rule settings it does use.
- Chapter 1 rests on `7|a|a` as well as `7|a|a|b`, because `7|a|a|b` (原始设定) has no rests-on path to the setting card itself.
- Hidden forward fault fixed by moving: the figure 图示：K 随噪声方差变化，以及误差两项此消彼长 (former 7\|a\|a\|4\|a) draws the two terms of the error (20), which the lemma's proof chain derives; it moved from under the result K to under the lemma (`7|a|a|2|b|b`).
- With results now ahead of their proofs, `check-settings` found seven symbols first met in a result card. Each result card already defines its symbol ($1-\Gamma_i$, $\varrho_i$ in 2\|b; $\Lambda_\delta$ in 5\|c; $\mathcal M$ in 6\|d; $\zeta_i,\xi_i$ in 6\|e; $\mathcal R_i$ in 6\|f), except $\Upsilon$, for which 5\|b got one defining clause. The notation rows now name those result cards. The chapter 2 card writes the gain and the pre-communication belief in words, so $K$ and $m_t$ are still first met in the proof chain, as their rows say.
- Chapter numbers in the setting part's text: the notation card's γ row (第 4 章 → 第 5 章) and the **说明** of `7|a|a|c` (第 5 章 → 第 6 章).
- The Model Card: new title (CRANK 模型：带沟通规则的不完全信息新凯恩斯模型), a model-wide 问题/概要, **结构** rebuilt from the tree, **分部** (设定与直觉 from 7\|a\|a; 模型构件 from 第 2 章; 需求与均衡 from 第 5 章). Its three stale 对照 lines to the chapters (written when 7\|a held only the household block) were dropped; its source line and cross-book line stay.

The **结构** sections of 4\|a, 5\|a, 6\|a and 6\|a\|28 were rebuilt from the tree as well (main line and one level under it).

### Gate

- `check-model.mjs`: 4\|a 31 members (6 main, 25 side), 5\|a 8 (2, 6), 6\|a 52 (8, 44), 6\|a\|28 32 (7, 25), 7\|a 85 (8 main, 77 side); every one loose 0, no closure, forward or unanchored fault, `self_sufficient` true.
- `check-network.mjs`: `network: clean` for all five (advice only: chapter 1 of 5\|a and chapter 1 of 7\|a have no `结果：` card, right for a single-derivation chapter and the intuition chapter). 7\|a's network: 设定 → {1, 2, 3, 4, 5}; 5 → 6 → 7.
- `check-premises.mjs`: 6\|a\|28 none; the other hits are pointers, kept with this reason:
  - 4\|a, 2 hits: the 变量表 rows name equations (1) and (2) of the specification cards.
  - 5\|a, 3 hits: the assumption's `怎样进入模型` line and the 说明 of (3) point to the equations the settings feed (stage 2 listed them).
  - 6\|a, 4 hits: the measure card, the hypothesis and two assumptions name the estimating equation (7) of the baseline specification; the evidence spine fixes the order data → hypothesis → design → specification, and the cards state the symbols of (7), not its estimates.
  - 7\|a, 9 hits: `7|a|a|g`, `g|a` and `h` are setting 说明 lines that say where (18), (20), (32), (36), (46) are derived (stage 2 listed them); `7|a|a|1|a` is the intuition chapter's preview of (30); `7|a|a|2|b|a|1` matches "AR(1)"; `7|a|a|6|e|a` is a definition's `怎样进入模型` line naming (38)(39)(43), where Δ and Δ_i are used.
- `check-settings.mjs`: 4\|a, 6\|a, 6\|a\|28 `settings: the data tree and the specification ladder are in place`; 5\|a, 7\|a `settings: all in the setting part, as a dependency tree`.
- `list_broken_links` (agent): empty. Literature tree 722 → 725 Cards.
- Term relink: `{"cards_changed":24,"links_added":109,"book":"assenza2026louder","rounds":1,"cards_refused":0,"ok":true,"applied":true,"links_repaired":0}`. The 7\|a **结构** was rebuilt once more after the figure move, and a second relink took its Term links back: `{"links_added":18,"cards_changed":1,"rounds":1,"links_repaired":0,"applied":true,"ok":true,"cards_refused":0,"book":"assenza2026louder"}`. All checks above were rerun after it and stay clean.

### Open

1. Step 3 card text was not redrafted: result cards keep their old body (they rest on their last proof card by `依赖`, without the `**为什么**`/`**证明**` sections of the result format), and derivation cards keep their digest format. Chapter cards 3–6 keep their old 问题/概要/关联 text below the new sections; chapter 6's 概要 still summarizes the zero-lower-bound results that now form chapter 7.
2. `7|a|a|g|a` (setting part, untouched) still points to the Model Card 7\|a for the posterior (18) and the error (20); both are now in chapter 2 (`7|a|a|2`).
3. The 6\|a hypothesis and 6\|a\|28 hypothesis still say `本模型检验 …`; the empirical chapter cards were not rewritten beyond their leads.
4. Tree unlocked throughout; no `card_locked` met. Other sessions wrote other books on the box at the same time; no read-view error occurred.

## Stage 4: imports and explanations

Date 2026-11-14, live box, under `zt-model-import` steps 1–8, done in one side session by scripts (no subagent; one DeepSeek job, for the explanation refresh). Backup: `D:\zt\backups\zt-before-assenza2026louder-imports.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Text ran as `edit_card` calls under `-Allow` (assenza2026louder Cards only). A script compared the display formulas and `\tag` numbers before each text write; no display formula, equation number or estimate changed. No `card_locked`, no `read_view_changed` and no failed batch occurred. Order: 5\|a, 4\|a, 7\|a, 6\|a, 6\|a\|28. Before any edit, `check` exited 0 and `plan` said `nothing to import` on all five heads: the two estimations named the theory and the measure only with `参照`/`对应` Links and `见` pointers.

### 5\|a (formal)

- No import. The rule (3)–(4) is stated for any series $\Delta e_t$; its setting card names the entropy series of 4\|a in words and the Model Card points to 4\|a with a plain reference, so no part rests on 4\|a. The measurement enters where the estimations bind the rule to data (below). Tolerated 0, false shares 0.
- 5\|a now lends the whole model to 6\|a and 6\|a\|28 (`lent`).

### 4\|a (empirical)

- No import: the measurement and its descriptive facts are the paper's own, and no other model in the box holds the series. It lends `4|a|a|d` (with its closure 4\|a\|a\|c, 4\|a\|a) to 6\|a and 6\|a\|28. Tolerated 0, false shares 0.

### 7\|a (formal, CRANK)

- No import. Rule (24) is the CRANK model's own setting (`7|a|a|f|a|b`, a log-level rule in $\exp(\Delta e_t)$ with $-\psi_y$); it corresponds to 5\|a's function but is no copy of it and rests on nothing in 5\|a. The correspondence stays a comparison: `7|a|a|4|b` (`对应 [[5|a]]`, the sign convention of (24) against the estimated rule). Tolerated 0, false shares 0.

### 6\|a (empirical: US rule estimation)

Premises set first (the plan finds only rests-on Links):

- 规格：基准 `6|a|a|3` now rests on `依赖 [[5|a|a|c]]` (the partial adjustment (3), kept with one lag) and `依赖 [[5|a|a|d]]` (the target (4) with $k=q=1$): equation (7) is (3) and (4) combined.
- 假说 `6|a|a|1`: `参照 [[5|a]]` became `依赖 [[5|a|a|d]] 的目标值 (4)`: the two signs are the signs of the coefficients $\psi_\pi$, $\psi_y$ of (4). A first wording that also named `[[5|a]]` on that line was read as resting on the whole Model Card (`plan`: by hand, whole model); the Model Card Link was dropped from the line.
- 数据 `6|a|a`: **起点** `原始设定` became `依赖 [[4|a|a|d]]`: the dependent variable is that card's quarterly statement entropy, taken in first log differences.

Import lines (`plan` → `apply` wrote `单卡` 4\|a\|a\|d, 5\|a\|a\|d, 5\|a\|a\|c; round 2 upgraded 4\|a\|a\|d to `整块`; the two 5\|a lines were then replaced by hand with one whole-model line, because an estimation imports the theory it estimates whole, as velic2025inflation 8\|a ← 3\|a):

- `整块 [[assenza2026louder/5|a]]`: 本模型估计的整个沟通反应函数，部分调整 (3) 与目标值 (4) 合成式 (7)，假说取 (4) 中两个系数的符号. Absorbs the two `单卡` lines; brings 8 parts (setting tree, notation card, chapter 1 with the steady state).
- `整块 [[assenza2026louder/4|a|a|d]]`: 因变量的来源，按式 (1) 逐份计算、加总到季度的 FOMC 声明熵序列. Brings the measure 4\|a\|a\|d and its closure 4\|a\|a\|c (formula (1)) and the data card 4\|a\|a (the FOMC statements), as velic 10\|a ← 7\|a\|a. The whole 4\|a data tree (`整块 [[4|a|a]]`) was not taken: 6\|a uses the entropy series, not the sample split, the corpus counts or the length descriptions.

Tolerated 0 → 5 after round 1 → 4 after the upgrade → 0 with the whole-model line. False shares 0. Symbols (step 6): the 变量表 `6|a|a|a` got the line `沿用 [[5|a|a|a]] 的符号。规则只留一阶滞后，本表的 $\rho_{e}$ 即那张卡的 $\overline{\rho}_{e}$；预测期取 $k=q=1$。` No copy found by `check`. The Model Card's **关联** lines `对应 [[4|a]]` and `参照 [[5|a]]` are Links to whole Model Cards (no false share) and stay.

### 6\|a\|28 (empirical: cross-country estimation)

- Premises: 规格：基准 `6|a|28|a|3` rests on `5|a|a|c` and `5|a|a|d` (the per-country rule is (3) with one lag and (4) with $k=q=1$); 假说 `6|a|28|a|1`: `参照 [[5|a]]` → `依赖 [[5|a|a|d]] 的目标值 (4)`; the measure `6|a|28|a|b`: the sentence `测量细节见 [[4|a]]，本模型沿用同一构造` became `声明熵沿用美联储声明的同一构造。` and its **起点** gained `依赖 [[4|a|a|d]]：各国声明的熵按同一构造逐份计算（式 (1)），再加总到季度`.
- Import lines (same rounds as 6\|a):
  - `整块 [[assenza2026louder/5|a]]`: 本模型在五家央行上估计的沟通反应函数，部分调整 (3) 与目标值 (4) 是基准规则，(9) 在 (4) 上加汇率项与美国利率项.
  - `整块 [[assenza2026louder/4|a|a|d]]`: 声明熵的构造，各国声明按同一方法逐份计算 (1) 再加总到季度. Its closure brings the FOMC data card 4\|a\|a, on which the construction is defined; 6\|a\|28's own sample stays its data card 6\|a\|28\|a.
- Not imported from 6\|a: the model re-estimates the same rule on other data (the fold decision); `参照 [[6|a]]` on its Model Card stays a comparison for the evidence dialogue. Its design assumption 5 (`6|a|28|a|2|e`, $\Delta_e^*$ and $\pi^*$ not separately identified) states for its own samples what 6\|a\|a\|2\|d states for the US; `check` lists no copy, and each estimation keeps its own normalization. Tolerated 0, false shares 0. The 变量表 `6|a|28|a|a` got the same `沿用 [[5|a|a|a]]` line.

### Stage-3 leftovers fixed (no formula change)

1. Chapter 6 `7|a|a|6`: the 问题 no longer asks about the zero lower bound (`正常时期的局部确定条件是什么。`); the 概要 sentence on (47)/(48) became `零利率下限处的确定性条件 (47) 与双重使命的作用 (48) 不在本章，归第 7 章。`; the velic 对照 line says `本模型第 7 章让零利率下限改变确定性条件`. The inline $\Theta+\kappa/\gamma$ and $(-1,1)$ of the old sentence went with it (no display formula).
2. `7|a|a|g|a` **说明**: `式 (11) 的冲击分解见设定卡 [[7|a|a]]。后验 (18) 见第 2 章的 [[7|a|a|2|a]]，信号提取误差 (20) 见同章的 [[7|a|a|2|b|a|3]]。` (was: all three `见 [[7|a]]`).
3. The two hypothesis cards: `这两个符号是本模型检验的预测` → `这两个符号是美国沟通规则估计检验的预测` (6\|a\|a\|1); `本模型检验同一条规则是否在五个经济体都成立` → `跨国沟通规则估计检验同一条规则是否在五个经济体都成立` (6\|a\|28\|a\|1).

### Explanation refresh (step 8)

- `explain.mjs plan`: 7\|a 5 Derivation parts in 1 job; 5\|a, 4\|a, 6\|a, 6\|a\|28 `nothing to explain`.
- One DeepSeek conversation (`opencode-go/deepseek-v4.1-flash`, effort high, the box's drive model `ZT_DRIVE_MODEL`; OpenCode Go account Primary, 11 requests, 168 s, context 66k of 1M; tools: 6 pwsh, 3 read, 2 write of which 1 failed and was retried; only `out/explain-01.json` was written): answer `{"done":5,"blocked":0}`. Cost: OpenCode Go subscription, the Harness ledger books 0 CNY for the source (no per-job token split; the 2026-11-13 opencode-go day line is shared with the IMEN1 queue). Conversation f3a28942 deleted, workspace removed.
- `apply`: `applied 5, refused 0, blocked 0, follow-up jobs 0`. `finish`: `explained 5 part(s); Term links changed 0 Card(s), second plan 0; left 0; refused 0` (exit 0); parts 7\|a\|a\|3\|a\|a, 3\|a\|a\|1, 3\|a\|a\|2, 3\|a\|a\|3, 3\|a\|a\|4 (the chapter-3 pricing chain). A later `plan` on 7\|a: `nothing to explain`.

### Gate

- `imports.mjs check`: exit 0 on 5\|a, 4\|a, 7\|a, 6\|a, 6\|a\|28; `plan`: `nothing to import` on all five.
- `get_model`: `self_sufficient: true` on all five; tolerated 0 and false shares 0 everywhere; 5\|a 8 parts (lends the model to 6\|a, 6\|a\|28), 4\|a 31 (lends 4\|a\|a\|d to both), 7\|a 85, 6\|a 63 (52 own + 11 shared), 6\|a\|28 43 (32 own + 11 shared).
- `check-settings.mjs`: 5\|a, 7\|a `settings: all in the setting part, as a dependency tree` (advice only: 5\|a 1 and 7\|a 4 `uses an assumption` lines, unchanged from stage 3); 4\|a, 6\|a, 6\|a\|28 `settings: the data tree and the specification ladder are in place` (no false `variable_uncovered` with the imported 4\|a\|a).
- `check-model.mjs`: loose 0, `self_sufficient true` on all five (6\|a `{"shared":11,"main":8,"side":44}`, 6\|a\|28 `{"shared":11,"main":7,"side":25}`); `check-network.mjs`: `network: clean` on all five.
- `list_broken_links` (agent): empty.
- Term relink: `{"book":"assenza2026louder","ok":true,"rounds":1,"cards_changed":2,"cards_refused":0,"links_repaired":0,"links_added":2,"applied":true}`. All checks above were rerun after it and stay clean.

### Open

1. The cross-book comparisons (5\|a `对照 [[velic2025inflation/3|a]]`, the riboni2026presidential contrasts on 5\|a\|a, the 6\|a/6\|a\|28 estimates) are for the evidence dialogue (stage 6); 6\|a\|28's `参照 [[6|a]]` likewise.
2. Two shell-written edits lost TeX backslashes (`$\rho_{e}$` came out as a carriage return plus `ho`); found by reading the stored card and rewritten at once from a file written with `String.raw`. No other card carried the fault (the other edits were checked by reading back).
3. 6\|a\|28 imports the FOMC data card 4\|a\|a as closure of the measure; a reader of the GUI board sees it on the 共用 shelf. Accepted: the construction is defined on it.
4. Stage-3 open item 1 (result and derivation card text not redrafted; chapter cards 3–6 keep their old 问题/概要 below the new sections) stays, except the chapter 6 summary fixed here.

## Stage 5: illustrate

Date 2026-11-14, live box, under `zt-model-illustrate`, done in one side session by scripts. No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-assenza2026louder-illustrate.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Only the formal models 5\|a and 7\|a were illustrated. The empirical models 4\|a, 6\|a and 6\|a\|28 have no figures (owner, 2026-11-11). Modules: `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\assenza2026louder-5_a.mjs` and `…\assenza2026louder-7_a.mjs`. Before the live write, each figure was rendered locally from the module (a copy of `verify.mjs` that reads the figure from the module instead of the Card) and the PNGs were inspected; layout faults (overlapping labels, a dashed arrow through a box, a clipped legend, a nested superscript that `m()` cannot typeset) were fixed there. `apply.mjs --write` then wrote 5 + 16 Cards, and a second write reworded one 直觉 line on 5\|a\|a (R4). No display formula, equation number or text outside the chart and the **直觉** section changed (apply guard). Every Model Card figure is built in three parts (`part()`, top-level `data-part`), so the chart sheet shows each as a tab.

### Figures

- Model Card 5\|a (**图**, before **结构**): (一 · 决策与时序) the FOMC as the one decision maker: information set $\mathcal I_t$ → two expectations → target (4) → partial adjustment (3) → the statement, whose change becomes next period's lag; solution order (第 1 章 the steady state; the coefficients estimated by the two empirical models); (二 · 因果结构) expected inflation gap, expected output gap, the steady-state value and the past changes → target $\Delta e_t^*$ and the autoregressive term → $\Delta e_t$ (weight $1-\bar\rho_e$) → the long run (第 1 章) and the estimation (6\|a), dashed feedback of $\Delta e_t$ into the lags; (三 · 模型研究的变化) five panels (expected inflation above target, expected output gap, gaps back to zero, stronger gradualism, the estimation).
- 5\|a setting cards (4), at the end of **说明**: a (the time line with the forward windows $k$, $q$ from $t$ as 示意, the chain committee → $\mathcal I_t$ → expectations → target, the dependent variable as the difference of two statements' log entropy); b (the four assumptions, each as what it restricts: lag weights summing to $\bar\rho_e$, the two coefficients, the long-run value, the chair level removed by differencing); c ((3) as a weighted sum, the weight bar $\bar\rho_e$ / $1-\bar\rho_e$ as 示意, the number line of $\bar\rho_e$ and the adjustment speed); d ((4) term by term, the primitives that move each term, the target entering (3)).
- Model Card 7\|a: (一 · 博弈与时序) shocks realize → households see $\boldsymbol z_t$ → the central bank sees $\tau_t,\pi_t,y_t$, sets (23) and (24), publishes $\boldsymbol s_t$ → households choose under their posterior → firms price under Rotemberg, $c_t=y_t$; solution order 第 2–7 章, 第 1 章 verbal; (二 · 因果结构) shocks and rule coefficients → communication effort, signal noise, nominal rate → signal-extraction error (mean-distortion channel), risk component (risk channel), real rate → demand (30) → inflation (31) → dynamics (第 6 章) and the zero lower bound (第 7 章), dashed feedback of current values and expectations to the two rules; (三) nine panels (Proposition 1, mean distortion, substitutability, Proposition 3, Proposition 4, Proposition 5 with Corollary 1, Corollary 2, Corollary 3, the zero lower bound of 第 7 章). Each box opens its Card.
- 7\|a setting cards (13): a (the four participants, the two terms of (10), the split of $\widehat\theta_t$ and $\widehat x_t$ into persistent AR(1) and transitory parts, a sample path as 示意); b (who sees what as a ✓/✗ table, the public signal, the two rules and the firms, the four-step timing of one period); c (the two observations of $\boldsymbol\tau_t$ with their noise, the two-step update prior → $G_i$ → $k_i$ → posterior with $\Gamma_i$, $\sigma_\eta^2(\Delta e_t)$ strictly decreasing as 示意); d (sources and uses of (21), the demand shock and inflation in the bond return); e (labour → variety → aggregator → consumption good, number lines $\theta_t>1$, $\alpha\in[0,1)$); e\|a (the two terms of the profit, the quadratic cost around $\pi=1$ for a small and a large $\Psi$, 示意); f (who observes $\tau_t$, the within-period order, the steady state); f\|a (the time line: (23) on realized $t$ values, (24) on expectations of $t+1$); f\|a\|a ((23) as a product, number lines $\phi_\pi,\phi_y\ge0$, a box opening the variant h); f\|a\|b ((24) as a product with $+\psi_\pi$ and $-\psi_y$, the two directions, the number line); g (the information set as a dashed frame: realized shocks and the signal in, $\tau_t$ out, $\Delta e_t$ setting the signal variance, posterior moments → consumption); g\|a (lognormal, first order, moments only; a posterior density with its mean and width, 示意); h (the rule rate against inflation with the floor $R_t=1$, baseline dashed, variant solid, 示意; baseline against variant).

### 直觉

The **直觉** section was written or rewritten on all 17 setting cards (5\|a: a, b, c, d; 7\|a: a, b, c, d, e, e\|a, f, f\|a, f\|a\|a, f\|a\|b, g, g\|a, h) and on the two 定义 parts 7\|a\|a\|5\|a (risk component) and 7\|a\|a\|6\|e\|a ($\Delta$, $\Delta_i$). Five setting cards had none (5\|a\|a\|c, d; 7\|a\|a\|h, f\|a\|a, f\|a\|b); the digest's sections on the others were one short clause each, and three named results or later equations in 怎样进入模型 (5\|a\|a\|b: (3)/(4); 7\|a\|a\|5\|a: what Proposition 1 proves; 7\|a\|a\|6\|e\|a: (38)(39)(43)). The new sections name chapters instead (第 1–7 章), with no Link, no result and no bare assumption name. `check-premises` hits fell from 3 to 1 (5\|a) and from 9 to 8 (7\|a) as a result.

### Chart fixes outside the setting part

`verify.mjs --model` also lints the digest's own charts. Two failed: 5\|a\|a\|1\|a\|a (no axis labels) and 7\|a\|a\|3\|a\|b (raw `mc_t`, `π_t`, `θ_t` in SVG text, no axis labels). Only their chart fences changed (script `fixcharts.mjs` in the session scratchpad, guard: text outside the fence identical): the subscripts became tspans, the axis texts became labels with `data-axis` (时期 t / 熵的变化 Δe; 实际边际成本 mc_t / 总通胀 π_t), and the Δe* label moved inside the frame. No curve and no number changed. The other four digest charts of 7\|a (7\|a\|a\|2\|b\|b, 6\|d\|b, 6\|h\|a, 7\|a\|b) passed.

### Gate

All lines below are the reruns after the Term relink.

- `verify.mjs --model assenza2026louder/5|a` (light and dark): `6 chart(s) in 9 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets. The PNGs, one per part, were inspected.
- `verify.mjs --model assenza2026louder/7|a` (light and dark): `19 chart(s) in 86 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets. The PNGs, one per part, were inspected.
- `apply.mjs` rerun: `same` for all 5 Cards of 5\|a and all 16 Cards of 7\|a; the chart-fix script: `same` for both Cards.
- `check-settings.mjs`: 5\|a and 7\|a `settings: all in the setting part, as a dependency tree` (advice lines unchanged from stage 4).
- `check-model.mjs`: 5\|a `relations {"refers":8,"rests-on":10,"illustrates":1}; loose 0; self_sufficient true`; 7\|a `members 85 {"main":8,"side":77}`, `relations {"refers":79,"rests-on":115,"contrasts":3,"corresponds":4,"illustrates":7}; loose 0; self_sufficient true`.
- `check-network.mjs`: `network: clean` on both.
- `check-premises.mjs`: 5\|a 1 hit, 7\|a 8 hits, all existing **说明** pointers (Open 3).
- `zt-check --cards` on the 23 written Cards: `faults: 6`, all R3 in existing text (Open 2). None is in a 直觉 section or a chart. The one new fault (R4, passive 被 in the 为什么需要 line of 5\|a\|a) was reworded away.
- `list_broken_links` (agent): `{"broken":[]}`.
- Term relink: `{"book":"assenza2026louder","cards_changed":11,"cards_refused":0,"links_added":13,"links_repaired":0,"applied":true,"rounds":1,"ok":true}`. All checks above were rerun after it and stay clean.

### Open

1. Stage 7 (literature organize) will renumber addresses. zt 0.63+ rewrites chart `data-zt` targets with the Links on a move, but each module's `head` must be changed to the model's new address and `apply.mjs --write` rerun on the new head after the organize (as the bergin2026exchange follow-up did), then `verify.mjs --model` must show 0 dead targets. The two digest charts fixed by script have no click targets.
2. The R3 faults that `zt-check` reports on the written Cards (semicolons) are all in existing text: the 7\|a summary, 7\|a\|a (item 3 and **说明**), 7\|a\|a\|c **说明**, 7\|a\|a\|g item 3, 7\|a\|a\|h **起点**. They predate this pass and were not changed.
3. The 8 remaining `check-premises` hits on 7\|a and 1 on 5\|a are the existing **说明** pointers listed at stage 3; none is in a figure or a 直觉 section.
4. No `card_locked`, no `read_view_changed` and no failed write occurred. Other sessions wrote other books on the box throughout.

## Stage 6: evidence dialogue

Date 2026-11-14, live box (owner's request), under `zt-evidence-dialogue` steps 1–5, done in one side session by scripts (`dialogue.mjs survey | apply | check`) and one DeepSeek judging conversation. No subagent ran. Backup: `D:\zt\backups\zt-before-assenza2026louder-dialogue.sqlite3` (`VACUUM INTO`, 396 MB, `PRAGMA integrity_check` ok). The Terms were relinked at the end of stage 5, so the survey ran without a first relink. Only the empirical models 4\|a, 6\|a and 6\|a\|28 took this pass; the formal models 5\|a and 7\|a are not on the dialogue's other side. Only Cards of assenza2026louder were written.

### Survey

The models were surveyed one at a time, each after the previous one was applied, so the judged record dropped the mirror pairs.

- `survey 4|a`: 5 estimates of ours, 1,368 Cards of the other side, estimand Terms 沟通努力 (1\|a\|a), 词汇多样性 (1\|a\|h), 1\|a\|i, 1\|a\|e, 对数熵 (1\|a\|f). 88 pairs in 8 messages: the four descriptive estimates of Table 1 (4\|a\|a\|3\|b–e: mean log entropy 1.65 / 1.70 / 0.87, standard deviation 0.85) against each of 22 estimates of 6\|a and 6\|a\|28, all paired on the book's own Terms 沟通努力 and 对数熵.
- `survey 6|a`: 13 estimates, 1,360 Cards, estimand Terms 1\|a\|a, 1\|a\|ci, 1\|a\|f. 47 pairs in 4 messages: 3 against 4\|a\|a\|4\|b (the entropy time series) and 44 between the US rule estimates (6\|a\|a\|4\|d, 4\|e, 6\|g–6\|j, 6\|m, 7\|a, 7\|b) and the five-country estimates (6\|a\|28\|a\|4\|b–f, 6\|a–6\|d).
- `survey 6|a|28`: 9 estimates, 1,364 Cards, estimand Terms 1\|a\|b, 1\|a\|y, 1\|a\|f, 1\|a\|ci, 1\|a\|cv. 13 pairs in 2 messages (the Canadian ψr estimate 6\|a\|28\|a\|6\|d against the US estimates, the two ψΔs estimates against ρe).
- **No cross-book pair.** As in bergin2026exchange, the survey pairs on shared Term addresses, widened only by zt-term-sync Links, and no Term Card of assenza2026louder links a Term Card of another book (checked on 沟通努力 1\|a\|a: no outbound or inbound Link from another book). A text search of the other books' Finding Cards and Model Cards confirms that the other side is thin for this estimand: no Finding Card names 央行沟通 or 熵; riboni2026presidential has 90 model parts and 20 Finding Cards that name the FOMC, but they estimate the policy stance of appointees (8\|a), not the communication effort; ozhan2026monetary, arrocannarsa2026flight and gomezcram2026government name the Taylor rule, which is the interest-rate rule, not the communication rule. No pair was forced.

### Judging (DeepSeek)

One conversation for the three surveys (the messages of 6\|a and 6\|a\|28 followed those of 4\|a in the same conversation, which kept the judge prompt as a cached prefix): `opencode-go/deepseek-v4.1-flash`, effort high (the box's drive model, as at stage 4), OpenCode Go account Primary (pinned), 14 requests, about 155 s of model time, context 149k of 1M at the end, no tool call. Cost: OpenCode Go subscription; the Harness ledger books 0 CNY for the source (the 2026-11-14 opencode-go day line is shared with other sessions, so there is no per-job token split). Conversation d1dc9c80 deleted, workspace removed.

One answer was wrong and was judged again: 6\|a p5 (US ψy 6\|a\|a\|4\|e against euro-area ψπ 6\|a\|28\|a\|4\|b) first came back `same: true` with the euro-area ψy number of another card. Asked to read the pair again, the judge answered `same: false` (different coefficients); only the second answer went to `apply`. Every other answer was checked against its pair's two titles.

### Pairs judged and written

| Head | Pairs | Same estimand | Agree / disagree / not comparable | Links written |
|---|---|---|---|---|
| 4\|a | 88 | 0 | 0 / 0 / 0 | 0 |
| 6\|a | 47 | 16 | 0 / 0 / 16 | 16 pairs, 32 lines (both sides agent) |
| 6\|a\|28 | 13 | 0 | 0 / 0 / 0 | 0 |

- 4\|a: the Table 1 moments of log entropy are levels of the text measure; every 6\|a and 6\|a\|28 estimate is a coefficient of the communication rule (ρe, π*, ψπ, ψy, ψΔs, ψr). Judged `same: false` throughout.
- 6\|a: the 16 same-estimand pairs set a US coefficient beside the same coefficient in another central bank's rule. All are `对照` (not comparable: different sample, and for the robustness rows also a different measure). The judge wrote the Canadian ψy (+1.37 against the US's negative values) as `对照`, not `反驳`, because the samples differ; the book itself states the Canadian sign is the exception.
- Pairs of different coefficients in the same rule (ψπ against ψy, ψr, ψΔs, π*), the decomposition 2.97 (7.04 − 4.07, 6\|a\|a\|7\|b) against any estimated coefficient, and the descriptive entropy series 4\|a\|a\|4\|b against any coefficient were judged `same: false` and wrote nothing.

The written Links (`对照`, in the **对应** section of both estimates, reason sentence quoting only the two Cards' numbers):

- ψπ, US against euro area 6\|a\|28\|a\|4\|b (3.82, s.e. 1.91): 6\|a\|a\|4\|d (7.04, s.e. 1.05), 6\|a\|a\|6\|g (CPI, 11.33), 6\|a\|a\|6\|j (Tealbook PCE, 8.48), 6\|a\|a\|6\|m (log length, 4.07).
- ψπ, US against Japan 6\|a\|28\|a\|4\|d (7.16, s.e. 3.06): 6\|a\|a\|4\|d, 6\|a\|a\|6\|g, 6\|a\|a\|6\|j, 6\|a\|a\|6\|m.
- ψy, US against euro area 6\|a\|28\|a\|4\|c (−0.27, s.e. 0.59): 6\|a\|a\|4\|e (−1.46, s.e. 0.68), 6\|a\|a\|6\|h (quadratic trend, −1.49), 6\|a\|a\|6\|i (k = q = 4, −1.43), 6\|a\|a\|7\|a (post-2008, −6.26).
- ψy, US against Canada 6\|a\|28\|a\|4\|f (1.37, s.e. 0.58): 6\|a\|a\|4\|e, 6\|a\|a\|6\|h, 6\|a\|a\|6\|i, 6\|a\|a\|7\|a.

`apply` lines: 4\|a `{"links":0,"written":0,"errors":0}`; 6\|a `{"links":16,"written":12,"errors":0}` (8 Cards of 6\|a, 4 of 6\|a\|28); 6\|a\|28 `{"links":0,"written":0,"errors":0}`. All 148 answered pairs are in the judged record (`~/.zt-evidence-dialogue/judged-default.jsonl`).

### Gate

All lines below are the reruns after the Term relink.

- `dialogue.mjs check`: 4\|a `{"broken":0,"proposed_again":0,"ok":true}`; 6\|a `{"broken":0,"proposed_again":0,"ok":true}`; 6\|a\|28 `{"broken":0,"proposed_again":0,"ok":true}` (exit 0 each).
- Every same-estimand pair is written as `对照` in **对应** on both sides (32 lines read back from the box); no pair was judged `印证` or `反驳`.
- `check-settings.mjs`: 4\|a, 6\|a, 6\|a\|28 `settings: the data tree and the specification ladder are in place`.
- `check-model.mjs`: 4\|a `members 31; loose 0; self_sufficient true`; 6\|a `members 63 {"shared":11,"main":8,"side":44}; loose 0; self_sufficient true`; 6\|a\|28 `members 43 {"shared":11,"main":7,"side":25}; loose 0; self_sufficient true`. The new `对照` Links run between two models, so they do not appear among a model's internal relations.
- `list_broken_links` (agent): `{"broken":[]}` before and after the relink.
- Term relink: `{"book":"assenza2026louder","ok":true,"rounds":1,"cards_changed":10,"cards_refused":0,"links_repaired":0,"links_added":13,"applied":true}` (Term links in the new reason sentences); `-PlanOnly` afterwards: `cards_changed 0, rounds 0`.

### Open

1. No cross-book pair: assenza2026louder's Terms (沟通努力, 对数熵, 沟通规则, …) link to no Term Card of another book. After a zt-term-sync pass on the economics shelf, survey the three heads again; the judged record keeps the 148 pairs judged here from being paid for twice. The likeliest neighbour is riboni2026presidential (FOMC text measures), but its estimates found so far measure policy stance, not communication effort.
2. The reverse lines on 6\|a\|28 read from the US side ("7.04 … 与 3.82"), as `apply` writes the judge's one reason sentence on both sides. Accepted: the numbers and the comparison are the same.
3. The cross-book comparisons on the formal model 5\|a (Stage 4 Open 1: `对照 [[velic2025inflation/3|a]]`, the riboni2026presidential contrasts) are not this skill's pairs and stay as written.
4. No script fault was found; no skill file changed. No `card_locked`, no `read_view_changed` and no failed write occurred. Other sessions wrote other books on the box throughout.

## Stage 7: literature organize

Date 2026-11-14, live box (owner's request), under `zt-literature-organize`, done in one side session by scripts (`outline.mjs`, `batch.mjs`, `leads.mjs`, `check.mjs`, and small `edit_card` scripts for titles and three heads). No subagent and no DeepSeek job ran, so there is no DeepSeek cost. Backup: `D:\zt\backups\zt-before-assenza2026louder-organize.sqlite3` (`VACUUM INTO`, `PRAGMA integrity_check` ok). Every batch was previewed first (`links_that_will_break` empty, complete true) and then committed under its plan hash. The tree went from 725 to 727 Cards (two new summary leads). No Card was deleted, and no formula, equation number or estimate changed. Only Cards of assenza2026louder were written. No `card_locked`, no `read_view_changed` and no failed batch occurred; the tree was unlocked throughout.

### Source divisions

From `D:\zt-digest\assenza2026louder\book.zh.md`: the title page (title, four authors, date 2026-10-30, 摘要, keywords, footnote 1), 引言, 1. 相关文献, 2. 实证分析 (2.1 with the run-in subsections 2.1.1 数据描述 and 2.1.2 以香农熵衡量词汇多样性; 2.2 with 2.2.1 前瞻性反应函数, 2.2.2 估计程序, 2.2.3 基准设定与数据, 2.2.4 基准估计, 2.2.5 稳健性分析; 2.3 国际证据), 3. 简单 RANK 模型中的系统性沟通 (3.1 环境 with 3.1.1 家庭, 3.1.2 中间品与最终品厂商, 3.1.3 货币当局; 3.2 动态 IS 方程; 3.3 均衡动态 with 3.3.1 正常时期 and 3.3.2 零利率下限), 4. 结论, then 参考文献. Only some of the third-level numbers are Markdown headings (2.2.2, 3.1.2); the others open a paragraph. They are the author's own numbered divisions, so they nest as the third level. The top line takes the numbered sections, with the title page first and 引言 as its own top Card (it is unnumbered but a top heading of the source).

### Final tree (top line and divisions)

```
1   前置页 (rewrite)                       a 封面与关键词 (a|a 脚注) · b 摘要 (b|a finding, b|1) · c 结构卡
2   引言                                   unchanged (2|a chain, findings 2|c–f)
3   1. 相关文献                            unchanged (retitled)
4   2. 实证分析 (new summary)             a §2 task (old 4|b)
                                           b 2.1 (old 4, rewritten): b|b 2.1.1 (old 4|b|1–4) · b|c 2.1.2 (old 4|b|5–13; Table 1 finding c|6|a, Figure 1 finding c|8|b) · b|d empirical model (old 4|a)
                                           c 2.2 (old 6, rewritten): c|a 2.2.1 (old 5; c|a|a notes old 5|a|1–4; c|a|b formal model old 5|a) · c|b 2.2.2 (old 6|a|1–4) · c|c 2.2.3 (old 6|a|5–6) · c|d 2.2.4 (old 6|a|7–9, findings old 6|b–g) · c|e 2.2.5 (old 6|a|10–26, finding old 6|h) · c|f empirical model (old 6|a)
                                           d 2.3 (old 6|a|27, chain old 6|a|29–35; d|a empirical model old 6|a|28)
5   3. 简单 RANK 模型中的系统性沟通 (new)  a §3 preamble (old 7|b)
                                           b 3.1 (old 7, rewritten): b|b §3.1 preamble (old 7|b|1) · b|c 3.1.1 (old 7|b|2–12, footnote 20 under the budget card) · b|d 3.1.2 (old 8|c–8|c|3) · b|e 3.1.3 (old 8, trimmed; old 8|c|4–8)
                                           c 3.2 (old 9)
                                           d 3.3 (old 10, rewritten): d|a §3.3 system (old 10|b–10|b|6) · d|b 3.3.1 (old 10|b|7–19) · d|c 3.3.2 (old 10|b|20–23)
                                           e formal model CRANK (old 7|a)
6   4. 结论                                old 11 (retitled)
7   术语表 (Term list, from 1|a)
8   参考文献 (Cited list, from 1|b)
```

Batches (tail to head, one division each): §3 (11 items, 151 renamed), §2.2–2.3 (19 items, 296 renamed), §2.1 (5 items, 195 renamed), front matter and lists (5 items, 343 renamed). Then `leads.mjs` twice: five rewrites, then two demotes.

### Crossers and how each was settled

- Old 1 held the front matter, the Term list and the Cited list: the two lists went to the tail (`7`, `8`); the abstract chain (old 1\|c\|1–2), which ran on the cover card's Direct line, became its own Side branch `1|b`, with the abstract finding (old 1\|d) under it.
- Old 4 led §2 and §2.1, and its body speaks for §2.1 only: demoted. A new §2 summary took its place, and old 4 is the §2.1 head (`4|b`, rewritten as a 2.1 summary). Old 4\|b (`§2, §2.1`, the four-step task) is the §2 preamble `4|a`; its Direct chain split into 2.1.1 and 2.1.2.
- Old 5 (titled 2.2) speaks only for §2.2.1, and old 6 summarized §2.2.2–§2.3 (a top Card that was two divisions at once). Old 6 became the §2.2 head and was rewritten to cover 2.2.1–2.2.5; its §2.3 content went into the §2 summary. Old 5 is the §2.2.1 head (`4|c|a`, retitled, its position line rewritten, a source line added).
- The model 6\|a carried 35 reading notes (§2.2.2–§2.3) and the model 6\|a\|28 on its Direct chain. The chain was cut by source line into 2.2.2, 2.2.3, 2.2.4, 2.2.5 and 2.3; 6\|a\|28 moved out as the 2.3 head's Side branch. The same held for 5\|a (four §2.2.1 notes on its chain, now under the 2.2.1 head) and for 6\|a\|28 (seven §2.3 notes and one event, now the 2.3 chain). No model member moved out of its model: `get_model` members stay 31, 8, 63, 43 and 85.
- Old 7 led §3, §3.1 and §3.1.1: demoted. A new §3 summary took its place; old 7 is the §3.1 head (`5|b`, rewritten as a 3.1 summary); old 7\|b (`§3`) is the §3 preamble `5|a`; old 7\|b\|1 (`§3.1`, the four agents) is the 3.1 preamble `5|b|b`; old 7\|b\|2–12 form 3.1.1.
- Old 8 summarized §3.1.2 and §3.1.3 with equal content on each side (five Cards each). The tie went to the later division: old 8 is the §3.1.3 head (`5|b|e`, retitled, its firm sentences and three firm `依赖` lines dropped, a source line added), and old 8\|c (the first §3.1.2 Card, which opens the section) heads 3.1.2.
- Old 10 already spoke for all of §3.3: rewritten in place (`5|d`); its single chain was cut into the §3.3 system cards, 3.3.1 and 3.3.2.
- Footnote 20 (old 8\|d, source line `§3.1.2`) explains the demand shock in the budget constraint (21) of §3.1.1; its source line gives the page where the note stands. Filed under the budget card `5|b|c|10|a`.
- Findings went under the notes that report them: Table 1 (`4|b|c|6|a`), Figure 1 (`4|b|c|8|b`), the five 2.2.4 baseline findings under the 2.2.4 head, the post-2008 finding under its note, the core-PCE finding under the 2.2.5 price-index note.

Calls that could go the other way: the third-level run-in subsections nested as divisions (rather than left flat under their second-level heads); old 6 as the 2.2 summary instead of a demote of old 5; the tie rule giving old 8 to 3.1.3; footnote 20 filed in 3.1.1 against its source line; 6\|a filed as the last Side branch of the 2.2 summary (it spans 2.2.2–2.2.5, as bergin filed its models), while 5\|a sits under 2.2.1 and 6\|a\|28 under 2.3, where they start and end; the CRANK model as the last Side branch of the §3 summary (it spans 引言 and §3).

### Summary leads and other text

- New summaries (`leads.mjs` demote): `4` (2. 实证分析), `5` (3. 简单 RANK 模型中的系统性沟通). Rewrites: `1` (前置页), `4|b` (2.1), `4|c` (2.2), `5|b` (3.1), `5|d` (3.3). Each has the source's name and main claim as title, a paragraph that walks the subdivisions, `小节索引：`, `在全书中的位置：` and the source line.
- Hand edits: `4|c|a` (2.2.1 head: title, position line, source line), `5|b|e` (3.1.3 head: title, text trimmed to 3.1.3, position line, source line). Division heads retitled with their section number: `3`, `4|b|b`, `4|b|c`, `4|c|b`–`4|c|e`, `4|d`, `5|b|c`, `5|b|d`, `5|d|b`, `5|d|c`, `6`.
- The root index lists the eight top Cards. The structure Card `1|c` now lists the six parts and the five models with their sections (its old model list named chapters of 7\|a as models, stale since the fold).
- `zt-check --cards` on the root and the 23 written heads: first run 6 faults (R4 in the root index line `是否被系统性设定`; R2 ×3 and R3 ×2 in `1|c`), reworded once; second run `faults: 0`, exit 0.

### Chart targets

Backup: `D:\zt\backups\zt-before-assenza2026louder-chart-targets.sqlite3` (integrity ok). In `C:\Users\chenx\.claude\skills\zt-model-illustrate\modules\assenza2026louder-5_a.mjs` the `head` is now `assenza2026louder/4|c|a|b`, and its two hard-coded targets of the estimation model (`assenza2026louder/6|a`, which after the organize names a 结论 note) now read `assenza2026louder/4|c|f`; in `assenza2026louder-7_a.mjs` the `head` is now `assenza2026louder/5|e`. The module file names stay as they are (the bergin module kept its old name too). `apply.mjs --write`: `same` for all 5 + 16 Cards, `nothing to write`: zt had already rewritten every `data-zt` target with the moves, including the two estimation boxes. Gate: `verify.mjs --model assenza2026louder/4|c|a|b`: `6 chart(s) in 9 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`; `verify.mjs --model assenza2026louder/5|e`: `19 chart(s) in 86 card(s); 0 lint fault(s); 0 card(s) without a figure; 0 setting card(s) without a full 直觉`. 0 dead targets on both (dead targets are a lint rule).

### Gate

- `check.mjs assenza2026louder`: top line 1 前置页, 2 引言, 3 1. 相关文献, 4 2. 实证分析, 5 3. 简单 RANK 模型中的系统性沟通, 6 4. 结论, 7 Term list (245), 8 Cited list (92); `no fault`. Against the start snapshot: 725 → 727 Cards, and three units smaller (5\|a 14 → 9, 6\|a 121 → 53, 6\|a\|28 41 → 33): the reading notes, the event and the nested model 6\|a\|28 that stood on the Model Cards' Direct chains, not model parts.
- Source-line walk (185 Cards with a source line): no reading note outside its division. The 12 lines that differ are model parts inside whole units (5\|e chapters, the model heads), footnote 20 (settled above) and the 2008 event under Figure 1, whose source line `§4.1` is a digest label. No crosser left.
- `list_broken_links` (agent): `{"broken":[]}`, before and after the relink.
- `check-model.mjs`: `4|b|d` members 31 (6 main, 25 side), `4|c|a|b` 8 (2, 6), `4|c|f` 63 (11 shared, 8 main, 44 side), `4|d|a` 43 (11 shared, 7 main, 25 side), `5|e` 85 (8 main, 77 side); loose 0 and `self_sufficient` true on all five.
- `check-network.mjs`: `network: clean` on all five.
- `check-settings.mjs`: `4|b|d`, `4|c|f`, `4|d|a` `settings: the data tree and the specification ladder are in place`; `4|c|a|b`, `5|e` `settings: all in the setting part, as a dependency tree`.
- `imports.mjs check`: exit 0 on all five.
- Term relink: `{"rounds":0,"book":"assenza2026louder","cards_changed":0,"cards_refused":0,"links_repaired":0,"applied":false,"links_added":0,"ok":true}` (`leads.mjs` had already turned the Term names of the new text into Term uses); `-PlanOnly`: `{"rounds":0,"cards_changed":0,"links_repaired":0,"ok":true,"book":"assenza2026louder","applied":false,"cards_refused":0,"links_added":0}`. No relink bug.

## Result

Final top line: 前置页 · 引言 · 1. 相关文献 · 2. 实证分析 · 3. 简单 RANK 模型中的系统性沟通 · 4. 结论 · 术语表 · 参考文献.

Models (final addresses):

- `assenza2026louder/5|e`, formal (level 2): CRANK, the incomplete-information New Keynesian model with a communication rule, 85 parts, setting card `5|e|a`, chapters `5|e|a|1`–`7`, illustrated; filed under the §3 summary.
- `assenza2026louder/4|c|a|b`, formal (level 2): the Fed communication reaction function (3)–(4), 8 parts, illustrated; filed under the 2.2.1 head. It lends the whole model to `4|c|f` and `4|d|a`.
- `assenza2026louder/4|b|d`, empirical (level 3): the entropy measure of communication effort, 31 parts; filed under the 2.1 summary. It lends the measure `4|b|d|a|d` to both estimations.
- `assenza2026louder/4|c|f`, empirical (level 3): the US rule estimation (iterated GMM), 63 members (11 imported); filed under the 2.2 summary.
- `assenza2026louder/4|d|a`, empirical (level 3): the cross-country rule estimation, 43 members (11 imported); filed under the 2.3 head.

All seven stages passed their gates. Open items across the sequence:

1. Restructure step 3 text was not redrafted in 5\|e: result cards keep their old body without **为什么**/**证明**, derivation cards keep their digest format, and chapter cards 3–5 keep their old 问题/概要 below the new sections (Stages 3–4).
2. R3 faults (semicolons) in existing setting text of 5\|e predate the illustrate pass and were not changed (Stage 5).
3. `check-premises` hits (1 on 4\|c\|a\|b, 8 on 5\|e) are existing **说明** pointers (Stages 3, 5); `check-settings` advice lines (`uses an assumption`) stay as advice (Stage 4).
4. The evidence dialogue found no cross-book pair: the book's Terms link to no other book's Term Card. Survey the three empirical heads again after a zt-term-sync pass on the economics shelf (Stage 6).
5. Notation row of $n_t(j)$ in 5\|e names the setting card instead of the firm setting (Stage 2 Open 5).
6. The illustrate modules keep their file names (`assenza2026louder-5_a.mjs`, `-7_a.mjs`) and their header comments still name the old addresses; only `head` and the two estimation targets changed.
7. The 2008 event `4|b|c|8|a` carries the digest source line `§4.1`, which the paper does not have.
8. This pass doc is not committed (the stage brief said no commit).

## Follow-up: open items fixed (2026-11-21)

Live box, by the owner's request ("solve them"), one side session (no subagent, no DeepSeek job). Box backup taken by the parent: `D:/zt/backups/zt-before-small-items-2026-11-21.sqlite3`. Only Cards of this book were written; no `card_locked` or `scope_refused` error.

- **Result item 5 (notation row of $n_t(j)$): fixed.** `check-settings.mjs` reads $n_t(j)$ as the symbol $n_t$ (it cuts a symbol at its argument), so a separate row pointing to the firm setting fails `first` (tried: `first: … a reader meets it first in H|a`). Following the skill's rule for one letter with two meanings, the notation card `5|e|a|a` now has one row `$n_t$、$n_t(j)$` whose meaning cell names both: aggregate labour $n_t$ in the setting card `5|e|a`, and variety labour $n_t(j)$, first met in the firm setting `5|e|a|e`; the 首次出现 cell stays `5|e|a`. The old `$n_t(j)$` row was removed.
- **Result item 7 (the 2008 event's `§4.1`): not changed, the line is correct.** `4|b|c|8|a` is an Event Card shared by three books. Its own line reads `来源：[[assenza2026louder]] §2.1.2`, which is the paper's real section (2.1.2 以香农熵衡量词汇多样性, where Figure 1 and the 2008 jump are discussed). The `§4.1` line is `来源：[[arrocannarsa2026flight]] §4.1`, the section of that paper (4.1 稳态识别, which anchors the calibration on the 2008 crisis). The stage-7 source-line walk read the other book's line; nothing in this book is wrong.

Cards changed: `5|e|a|a`.

Gate: `check-settings.mjs 5|e` `settings: all in the setting part, as a dependency tree` (no `first` fault; advice lines only); `check-model.mjs` 85 parts, `loose 0; self_sufficient true`; `check-network.mjs` `network: clean`; `check-premises.mjs` 8 hits (unchanged pointers); `imports.mjs check` exit 0; `list_broken_links` (agent) `{"broken":[]}`; Term relink `{"links_added":0,"cards_changed":0,"ok":true}`.

Left open: Result items 1–4, 6 and 8 (not in this brief).
