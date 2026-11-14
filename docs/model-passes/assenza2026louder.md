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
