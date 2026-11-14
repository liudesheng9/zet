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
