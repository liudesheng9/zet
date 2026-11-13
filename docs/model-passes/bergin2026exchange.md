# bergin2026exchange: post-digest model passes

Owner's order (2026-11-13), relayed by the IMEN1 queue session: run the zt-post-digest sequence on the live box while the queue digests assenza2026louder on the same box. Subagents run at effort medium. Digest run report: `complete: true` after the hand cleanup rerun on zt-digest 0.53.2 (bef70bcf).

## Stage 1: fold

Backup: `D:\zt\backups\zt-before-bergin2026exchange-fold.sqlite3` (`VACUUM INTO`, integrity ok).

Survey before: 14 Model Cards (7 formal, 5 empirical, 2 concept), 9 candidate groups.

| Model Card | Section | Model | Surviving head | Reason |
|---|---|---|---|---|
| 7\|a | §3.1 | two-country DSGE | 7\|a | formal; the goods-market block (CES indices, price indices, demand) of the paper's one theory model; first formal card, kept as head |
| 8\|a | §3.2 | two-country DSGE | 7\|a | formal; the household block of the same model under capital controls; rests on 7\|a's price index |
| 9\|a | §3.3–3.4 | two-country DSGE | 7\|a | formal; the traded-sector firm block (production, entry and exit, pricing) of the same model |
| 10\|a | §3.5 | two-country DSGE | 7\|a | formal; the government block (tax-financed reserve purchases, full sterilization) of the same model |
| 12\|a | §4.1 | two-country DSGE | 7\|a | formal; solves the same model for the reserve policy's effect on profits and firm value; rests on 9\|a and 10\|a |
| 15\|a | §5.1.2 | two-country DSGE | 7\|a | concept; the firm-location externality is the mechanism of the same model's simulation (home entry lowers the manufacturing price index); becomes a chapter |
| 17\|a | §5.2 | two-country DSGE | 7\|a | formal; a variant that changes one assumption (foreign firm value starts at the exit margin) and keeps the rest; rests on 9\|a, 10\|a, imports 12\|a |
| 18\|a | §5.3 | two-country DSGE | 7\|a | formal; a variant that changes one assumption (infinite planning horizon, T→∞) and keeps the rest; rests on 9\|a, 10\|a, imports 12\|a |
| 2\|a | §1 | two-country DSGE | 7\|a | concept; the introduction's verbal version of the formal model; becomes the model's intuition chapter |
| 6\|a | §2.3 | 45-country panel estimation | 6\|a | empirical; the full estimation (Tables 2–5: within and system-GMM productivity regressions, firm-dynamics regressions, IV), 65 parts; kept as head |
| 2\|b | §1 | 45-country panel estimation | 6\|a | empirical; the introduction's statement of the same estimand on the same data, repeating Tables 2–5 |
| 4\|a | §2.1 | 45-country panel estimation | 6\|a | empirical; the data and measurement built for this estimation, with its descriptive statistics; no other empirical model uses the panel |
| 5\|a | §2.2 | 45-country panel estimation | 6\|a | empirical; the productivity regressions (Table 2) of the same estimand and data |
| 5\|b | §2.2 | 45-country panel estimation | 6\|a | empirical; the firm-dynamics regressions and the undervaluation IV (Tables 3, 5) of the same estimand and data |

Two models: the two-country DSGE (formal, head 7\|a) and the 45-country panel estimation (empirical, head 6\|a). They stay two models: the panel tests the theory's prediction, and a theory and its test are never folded together.

### Fold (executed 2026-11-13, live box)

Method: one Card per run with a copy of the proven per-card fold script (`foldb.mjs`, from the velic2025inflation pass): strip the former head's **结构** and **引入**, mask every Link in its body whose target is a Model Card or Model part of any book (the cross-book Links of 9\|a, 4\|a and 5\|b to atkinson2015lectures, hart1995firms, velic2025inflation and becker2025division parts would otherwise block the retype), park its parts as Side successors of its section anchor, retype it (facet `第 N 章：…`, emphasis core, level cleared), move it `alone` into the head's main line, return the parts under it, and restore the stripped body with the rename map. Idea-note Direct successors (12\|a\|1–6, 15\|a\|1–7) and Idea Side cards (10\|a\|a, 5\|b\|a) went to the section anchor (now 12\|c, 15\|b, 10\|c, 5\|d). Each run was previewed without `--commit`, then committed; every retype and batch returned ok.

**引入** lines: every folded card's import lines (8\|a → 7\|a\|a\|7; 12\|a → 9\|a\|a\|7–9 and 10\|a\|b\|5; 17\|a and 18\|a → 9\|a\|a\|8–9 and 12\|a\|a\|9 or \|7) named parts that are now the head's own parts, so none was appended; 7\|a has no **引入** section.

Where each former head ended:

| Former head | Now | Facet |
|---|---|---|
| 2\|a | 7\|a\|a\|1 | 第 1 章：文字版本与直觉（§1）; placed directly after the setting card 7\|a\|a, before the goods-market block (now 7\|a\|a\|2–8) |
| 8\|a | 7\|a\|a\|9 | 第 2 章：家庭的跨期选择（§3.2） |
| 9\|a | 7\|a\|a\|10 | 第 3 章：贸易部门企业（§3.3–3.4） |
| 10\|a | 7\|a\|a\|11 | 第 4 章：政府储备政策（§3.5） |
| 12\|a | 7\|a\|a\|12 | 第 5 章：储备政策与企业进入（§4.1） |
| 15\|a | 7\|a\|a\|13 | 第 6 章：企业区位转移外部性（§5.1.2） |
| 17\|a | 7\|a\|a\|14 | 第 7 章：外国企业退出（§5.2） |
| 18\|a | 7\|a\|a\|15 | 第 8 章：无限规划期（§5.3） |
| 4\|a | 6\|a\|a\|8 | 第 5 章：数据、度量与描述统计（§2.1） |
| 5\|a | 6\|a\|a\|9 | 第 6 章：部门生产率回归（§2.2 表 2） |
| 5\|b | 6\|a\|a\|10 | 第 7 章：企业动态回归与低估 IV（§2.2） |
| 2\|b | 6\|a\|a\|11 | 第 8 章：引言的经验动机（§1） |

Each chapter card keeps its former parts as its Side successors (its former setting or data card is `<chapter>|a`, its limits card `<chapter>|b`).

Gate:

- `model-survey.mjs bergin2026exchange`: 2 Model Cards, 7\|a (formal, 81 parts) and 6\|a (empirical, 217 parts); one `read` pair 6\|a → 7\|a (kin 8, compare 6), the theory-and-test Links that stay between the two models. The tree still holds 685 Cards.
- `check-model.mjs bergin2026exchange/7|a`: 81 members (16 main, 65 side), every former head a part; no closure or forward fault; `unanchored` only on the eight new chapter cards 7\|a\|a\|1 and 7\|a\|a\|9–15 (the restructure fixes these); self_sufficient false for that reason only.
- `check-model.mjs bergin2026exchange/6|a`: 217 members (12 main, 203 side, 2 off: the limits cards 6\|a\|b, 6\|a\|c); no closure or forward fault; `unanchored` only on the four new chapter cards 6\|a\|a\|8–11.
- `list_broken_links` (agent): empty.
- Term relink: `{"rounds":1,"ok":true,"cards_changed":3,"links_added":4,"cards_refused":0,"links_repaired":0}`.

Duplicates the settings pass must merge:

- Formal 7\|a: each chapter carries its former head's own setting card (7\|a\|a\|1\|a, 7\|a\|a\|9\|a … 7\|a\|a\|15\|a); these are the model's assumptions and go to the setting part. 7\|a\|a\|1\|a (the verbal setting: home reserve policy, manufacturing firms, passive foreign country) restates the setting of 7\|a\|a\|12\|a and 7\|a\|a\|14\|a in words. 7\|a\|a\|14\|a and 7\|a\|a\|15\|a are variant settings (start at the exit margin; T→∞) and need their own branches. 7\|a\|a\|11\|a\|3\|a (part, 脚注 17) and the Idea card 10\|c state the same footnote.
- Empirical 6\|a: five data cards for the same 45-country panel (6\|a\|a, 6\|a\|a\|8\|a, 6\|a\|a\|9\|a, 6\|a\|a\|10\|a, 6\|a\|a\|11\|a), each with its own 假说, 设计, 规格：基准 and 变量表 chain; the measurement cards of 4\|a (under 6\|a\|a\|8\|a: labor productivity, extensive and intensive margins, domestic intermediate share, capital controls and reserve growth, listed firms) repeat 6\|a\|a\|b–d; the chapters of 5\|a, 5\|b and 2\|b repeat 6\|a's Tables 2–5 chapters (productivity, firm dynamics, IV and subsamples, firm-dynamics IV) and must merge into them; the limits cards (6\|a\|b, 6\|a\|c, 6\|a\|a\|8\|b … 6\|a\|a\|11\|b) overlap.

## Stage 2: settings (strict)

Date 2026-11-13, live box, under `zt-model-settings`, done in the main thread by scripts (no subagent, no DeepSeek job, so no DeepSeek cost). Backup: `D:\zt\backups\zt-before-bergin2026exchange-settings.sqlite3`. Moves and deletes ran as `apply_batch` tree batches (preview, then commit under the plan hash); text ran as `edit_card` calls under `-Allow`; every display formula of a rewritten setting card was compared with the old body by script before the write. The tree went from 686 Cards (685 + the new notation card) to 558: 7\|a lost one Card, 6\|a lost 127.

### 7\|a (formal)

The setting card stays `7|a|a`; the new notation card `7|a|a|a` (符号表与读法) is its first Side successor. Each folded chapter's setting card left its chapter; the derivations on its Direct chain moved up into the chapter's Side slot, so every chapter now opens on its own derivations (`7|a|a|1|a`, `8|a`, `9|a`, …). The goods-market assumption left the main line, so the chapters renumbered: 第 2 章 is now 7\|a\|a\|8, … 第 8 章 7\|a\|a\|14.

```
7|a|a   设定：两个部门、内生的品种数量与两层 CES 消费指数   原始设定
├ a  符号表与读法 (new; blocks: 两国与消费指数, 家庭, 企业, 政府与储备政策, 资本管制与模拟, 推导辅助符号, 术语)
├ b  设定：概述（§1） (from 7|a|a|1|a)                      原始设定
├ c  假设：家庭给定价格，以最小支出买到一单位指数 (from 7|a|a|2) ← 7|a|a
├ d  设定：家庭 (from 9|a)                                   ← 7|a|a, c
│    d|a 偏好 (10);  d|b 预算约束 (11)
├ e  设定：贸易部门企业 (15)(16) (from 10|a)                 ← 7|a|a, d
│    e|a 假设：企业同质且没有固定贸易成本
├ f  设定：政府 (from 11|a)                                  ← 7|a|a, d
│    f|a 非贸易部门技术 (24);  f|b 政府预算约束 (26)(27) → f|b|1 假设：封闭资本账户…;  f|c 储备政策 Ω_t (28)
├ g  设定：国际收支约束 (29) (from 12|a)                     ← d, e, f|c
├ h  设定：基准模拟（§5.1） (from 13|a)                       ← e, f|c
├ i  设定：变体一：外国从退出边际起步 (from 14|a)            变体一 ← h, e
└ j  设定：变体二：无限规划期 (from 15|a)                    变体二，与 i 并列 ← h, e
```

- Every setting card is in the format **起点** / **设定** (or **假设**) / **直觉** / **说明**; the six symbol tables and six term tables of the folded setting cards went into the notation card. The 11\|a\|1 and 11\|a\|3 cards (non-traded technology, government budget constraints) and the 11\|a\|5 definition of Ω_t became settings (facets `设定：…`); 10\|a\|4 (17)–(19), 10\|a\|8 (value) and 10\|a\|9 (bounds) stay in chapter 3 as derivation, definition and proposition.
- Statements moved: the 5% policy path and the symmetric initial steady state, stated by both variants, now stand once in the baseline simulation `h`; the variants state only their change and name `h` in **起点**. The verbal setting `b` keeps its seven items (it restates `f`, `h` and `i` in words; the check finds no repeat).
- Merge: the footnote-17 part 7\|a\|a\|11\|a\|3\|a was deleted `into` the assumption `7|a|a|f|b|1`, whose **说明** now lists the five simplifications and links the reading note 10\|c. 10\|c stays: it is the Literature tree's reading note of §3.5, not a model part.
- Small additions: `j` defines $v_t(h)$ at its first use; `g` defines $\mu_t$ and $D_{Tt}$; `7|a|a|8|b` (外国居民) gained `依赖 [[7|a|a|d]]` and is no longer loose.

### 6\|a (empirical)

The kept build is 6\|a's own: data card `6|a|a`, 变量表 `6|a|a|a`, 假说 `6|a|a|1`, 设计 `6|a|a|2`, 规格：基准 `6|a|a|3` with its ladder (15 specifications, each with its 估计式; every IV specification writes both stages), chapters 1–4 (`6|a|a|4`–`7`) and chapter 5 (`6|a|a|8`, §2.1, now holding only its limits card).

- Moved in (kept as distinct): 测量 国内中间投入份额 `6|a|a|e`, 测量 上市国内企业数 `6|a|a|f`, 描述 中国 `6|a|a|g`, and the descriptive statistics `6|a|a|h` (former 第 0 章, retitled 描述 with **起点**) with its eight means `6|a|a|h|a`–`h` (估计 → 描述: they rest on measures, not on a specification); 假设 6–10 under the design (`6|a|a|2|f`–`j`: two-way fixed effects, instruments not weak, CC index only policy, controls absorb sector shocks, policy variables exogenous in (6)); the Table 2 (2) AR and over-identification tests under `6|a|a|4|c` (`|b`, `|c`); the alternative-index test under `6|a|a|6|e`; the limits cards of the three dropped chapters to the Model Card (`6|a|d`, `e`, `f`).
- Text merged into the kept cards before the deletes: the data card took the extra source, sample-rule and observation-count items; the 变量表 took 17 measurement rows; the three measures `b`–`d` took the richer §2.1 text and the measurement assumptions (de jure index, trade threshold, splicing) as sentences, `d` the 补写 equations of ΔRSRV and CAP; the hypothesis took the non-manufacturing sentence and the links to 7\|a's chapters; the design took the five new assumption lines and two threats; 假设 3 took the AR reasoning. The outside-link lines of the dropped chapter cards (theory links, cross-book 跨书关联 to goldsmithpinkham2020bartik, becker2025division, velic2025inflation) were appended to the chapter or Model Card that took them.
- Deleted `into` the card each repeats (Links re-pointed, `links_that_will_break` empty in every preview): 127 Cards. §2.1 build 18 (data card, 变量表, measurement design and its five measurement assumptions, the seven measurement "规格", three measures) plus its two measurement tests; Table 2/4 build 43; Table 3/5 build 31; introduction build 33. That is 4 data cards, 4 变量表, 3 假说, 4 设计, 7 identifying-assumption duplicates (besides the five measurement assumptions), 40 duplicate specifications (7 of them the §2.1 measurement formulas), 3 chapter cards (第 6, 7, 8 章, into 第 1 章, 第 2 章 and the Model Card) with their sub-chapters, the table cards and the duplicate estimates and tests; every estimate whose number differs from the kept ones was kept (none was found by table column).
- A head line copied from the introduction chapter (`检验 [[7|a|a|1]] 与 [[7|a|a|12]] 的预测`) made those two 7\|a chapters legacy shared parts of 6\|a; it was removed again (the hypothesis carries the same links), so 6\|a shares nothing.

### Gate

- `check-settings.mjs bergin2026exchange/7|a`: `settings: all in the setting part, as a dependency tree` (advice only: the setting card has 9 settings filed under it).
- `check-settings.mjs bergin2026exchange/6|a`: `settings: the data tree and the specification ladder are in place`.
- `check-model.mjs 7|a`: 81 members (15 main, 66 side), loose 0, no closure or forward fault; unanchored on 7 chapter cards (7\|a\|a\|1, 8–12, 14; chapter 7 is now anchored).
- `check-model.mjs 6|a`: 90 members (9 main, 76 side, 5 off), loose 4 (limits cards 6\|a\|b, c, f and 6\|a\|a\|8\|a), no closure or forward fault; unanchored on chapter 5 (6\|a\|a\|8) only.
- `model-survey.mjs`: two Model Cards, one read pair each way (6\|a → 7\|a kin 5, compare 4).
- `list_broken_links` (agent): empty. Term relink: `{"rounds":1,"ok":true,"cards_changed":23,"links_added":86,"cards_refused":0,"links_repaired":0}`; the checks were rerun after it and stay clean.

### Open

1. 7\|a's goods-market derivations (7\|a\|a\|2–7) stand on the main line between chapter 1 and chapter 2; the restructure makes them a chapter. The derivation chain in chapter 4 (`7|a|a|10|a`) runs pricing (25) → taxes → proposition as one chain.
2. `check-settings` reads $l_t(h)$ and $e_t$ as first met in `7|a|a|d` and `7|a|a|e` (it matches $l_t$ and $ne_t$); the rows point where the check says.
3. 6\|a's chapter 5 (6\|a\|a\|8) keeps only a limits card after its descriptive statistics moved to the data tree; the restructure decides whether it stays. The six limits cards overlap in part (exogeneity of the policy variables in 6\|a\|d and 6\|a\|e) and were not merged.
4. The kept measure (4) writes $CC=1-KAOPEN$ without indices; the dropped §2.1 copy wrote $CC_{jt}=1-KAOPEN_{jt}$. The kept spelling stays.
