# Model passes: hart1995firms (2026-10-31)

Live box (`D:\zt`, service 127.0.0.1:4717). The tree was not locked, and no move or edit was refused for a pin by a locked tree. This record continues §13–13.2 of `docs/model-merge-proposal.md`, which folded the book into 19 models and gave the three folded models (`5|a`, `11|a|12`, `20|d`) their settings and spines. This pass covers the 16 unfolded models.

## Models

`model-survey.mjs hart1995firms` lists 19 Model Cards. Nine unfolded models are formal and got all three passes; seven are concept models with at most four own parts and stay as they are (the decision of `ebrill2001modern`: a chapter spine would break the concept level).

| Model Card | Model | Level | Own parts before → after | Passes |
|---|---|---|---|---|
| `3|a` | neoclassical firm (§1.1) | formal | 9 → 12 | settings, restructure, import |
| `3|b` | agency model (§1.2) | formal | 7 → 10 | settings, restructure, import |
| `9|a` | one-sided hold-up (§4.1) | formal | 13 → 17 | settings, restructure, import |
| `10|a` | message-dependent ownership (§A4) | formal | 11 → 14 | settings, restructure, import |
| `11|a|7` | Aghion–Bolton (§5.1) | formal | 13 → 17 | settings, restructure, import |
| `13|a` | costly state verification (§A5.1) | formal | 10 → 13 (+16 imported) | settings, restructure, import |
| `14|a` | model 1, debt and liquidation (§6.2) | formal | 15 → 18 (+1) | settings, restructure, import |
| `15|c` | model 2, long-term debt and investment (§6.2) | formal | 15 → 18 (+1 imported) | settings, restructure, import |
| `16|a` | model 3, short-term debt (§A6.1) | formal | 22 → 27 (+2 imported) | settings, restructure, import |
| `1|b` | house-building contracts | concept | 4 | skipped: concept model, 4 own parts |
| `3|c` | complete-contract benchmark | concept | 4 | skipped: concept model, 4 parts |
| `3|d` | hold-up | concept | 3 | skipped: concept model, 3 parts |
| `9|b` | ADR alternating offers | concept | 3 | skipped: concept model, 3 parts |
| `12|f` | multiple investors | concept | 4 | skipped: concept model, 4 own parts |
| `18|d` | cash-auction benchmark | concept | 4 | skipped: concept model, 4 parts |
| `18|e` | AHM procedure | concept | 4 | skipped: concept model, 4 parts |

(Own parts count the Model Card's own members, not imported ones. "After" adds one notation card and the new chapter cards.) Each skipped model passes `check-model.mjs` (`self_sufficient` true) and `imports.mjs check` (exit 0) as it stands. The survey lists `1|b` and `12|f` with five parts; one of the five is a Link from the Model Card to another model's part (a false share), so each has four own parts.

## Backups

`VACUUM INTO` before each pass, integrity ok, in `D:\zt\backups\`: `zt-before-hart1995firms-settings-<m>.sqlite3` and `zt-before-hart1995firms-spine-<m>.sqlite3` for each of the nine models (`<m>` = `3a`, `3b`, `9a`, `10a`, `11a7`, `13a`, `14a`, `15c`, `16a`), and `zt-before-hart1995firms-imports-<m>.sqlite3` for the models whose import pass changed a Card (`10a`, `11a7`, `14a`). After every check passed, all 21 backups went to the Recycle Bin.

## Settings pass (all nine)

Faults before: no model had a notation card or a **起点** section. Each setting card carried its own symbol and term tables. Three models had a setting filed in the main line: `3|a` (`假设：平均成本先降后升呈 U 形`), `3|b` (`假设：质量可订约，努力不可订约…`), `15|c` (`假设 1：y_1<i 且 y_2≥L`); `11|a|7` had its state-dependent variant (`扩展：收入依赖可验证状态 θ`) as a main-line step.

Changes:
- A new notation card `符号表与读法` as Side `a` of each setting: the setting's symbol and term tables moved out of the setting card, the marks legend added, and each row's 首次出现 code (`S`, `D1`, `本卡` …) replaced by a Link to the card that first uses the symbol.
- The four settings above moved `alone` into the setting part: `3|a|a|b` (`设定：成本曲线形状`), `3|b|a|b` (`设定：信息与风险态度`), `15|c|a|b` (`设定：假设 1`), `11|a|7|b|b` (`设定：状态依存`, title `设定：收入依赖可验证状态 θ`). Their rests-on Links to derivations and chapters became **起点** lines on the setting; `11|a|7|b|b` keeps a pointer to the random allocation it replaces in chapter 3.
- Every setting card got a **起点** section. Each base setting is `原始设定`, except `15|c|a` (rests on the imported `14|a|a`) and `16|a|a` (rests on the imported `14|a|a` and `15|c|a`).
- Symbols defined at first use: `3|a|a` defines $C(Q)$, $C(Q)/Q$ and $C'(Q)$ (the U-shape setting uses them before chapter 1 derives $C(Q)$); `3|b|a|b` defines the reservation utility $U$; `13|a|a|a` has a row for $\bar s(y,y)$, which `推导：核查区上交付额不超过 ŝ` introduces.
- Copies: the notation cards of `15|c` and `16|a` no longer restate `14|a|a`'s symbols ($y_1$, $y_2$, $L$, $P_1$, $P_2$, $R$); one line `沿用 [[14|a|a]] 的全部符号` (and for `16|a`, `15|c|a`'s $i$, $r$) binds them. `15|c|a`'s `参照 [[14|a|a]]` line became its **起点** premise. The `imports.mjs plan` copy advice on `15|c|a` is gone.

`check-settings.mjs --symbols` prints `settings: all in the setting part, as a dependency tree` for all nine.

## Restructure pass (all nine)

22 new chapter cards (facet `第 k 章：…`, emphasis core), created as placeholders and filed by `tree-mover.mjs` (248 moves in all). DeepSeek V4.1 Flash (official source, effort high) drafted 51 cards in 9 jobs under four Haiku managers: the 22 chapter cards (**直觉**, **本章结果**, **承上启下** with one `依赖` line), 19 result cards (claim first, **结果** with the original formulas, **为什么**, **证明**) and 10 asides. `check-drafts.mjs` found no lost formula, equation number or bad Link in any job. All conversations were deleted, and the nine job workspaces removed from the DeepSeek Desktop. Derivation cards keep their text.

| Model | Chapters (main line after the setting) | Results under them |
|---|---|---|
| `3|a` | 1 成本最小化给出成本曲线与边际成本; 2 价格等于边际成本决定企业规模 | cost curve (with the marginal-cost derivation); $p^*=C'(Q^*)$ with its chain, the replication argument and the limitation as asides |
| `3|b` | 1 努力可观察时固定支付使风险全归买方; 2 努力不可观察时激励与保险此消彼长 | first-best fixed payment (one derivation); the incentive problem, the trade-off, the limitation |
| `9|a` | 1 事后平分剩余使 M1 投资不足; 2 可描述或可验证时简单合同恢复一阶最优; 3 重新协商使任何长期合同都失效 | underinvestment (4.3)–(4.4); specific performance (4.5), cost sharing, contract (A); renegotiation (4.6)–(4.7) with three derivations, the commitment limit |
| `10|a` | 1 确定或随机所有权都不能让双方都投资; 2 期权合同支持一族混合策略均衡 | fixed ownership, random ownership; the option contract and the mixed-strategy family (chain (4A.1)–(4A.8)) |
| `11|a|7` | 1 E 控制时重新谈判实现一阶最优; 2 C 控制保证筹资，随机控制权居中; 3 高状态由 E 控制的临界值契约最优 | $y(a_E)\ge K$; (5.1) and the random allocation σ; the threshold θ* |
| `13|a` | 1 如实报告约束化为交付额的两条不等式; 2 最优契约是面值为 s* 的标准债务 | the program (5A.3)–(5A.4) with four derivations; proposition A1 with the random contract (A2), the commitment example and the limitations |
| `14|a` | 1 债务面值决定企业何时清算; 2 不确定性下最优债务取中间值 | benchmark (6.1), total return (6.2) with three derivations; the objective, the certain case, the two states, the conclusion with the chain to (6.3), the dividend-scheme comparison |
| `15|c` | 1 长期债务面值决定新投资能否融资; 2 两状态下长期债务挡住坏项目也可能挡住好项目 | proposition 1, (6.4)–(6.6) with the complex-security aside, the project-finance note; the certain case, the two states, the conclusion with four derivations |
| `16|a` | 1 短期债务抽走自由现金流，回报为三段式; 2 总现金流相等或子情形 1 时一阶最优可达; 3 子情形 2 在无效投资与破产之间取舍; 4 最优债务分三区，随有形性弱增 | (6A.1); the certain case, equal cash flows, (6A.2), sub-case 1; the two paths, the plan comparison, the bankruptcy plan; (6A.3)–(6A.5) and the comparative statics |

Other text changes:
- The `**分部**` sections of `9|a` (three parts) and `16|a` (four parts) pointed at cards that are no longer main-line starts; both models now have fewer than five chapters, so the sections were removed. Each head's **结构** was rebuilt (main line and one level under each main card).
- `3|a|a|1|a` and `13|a|a|1|a` are now labelled as results (`结果：成本曲线`, `结果：规划问题`); a stale `下一张 [[…]]` pointer in `3|a`'s marginal-cost derivation now names the setting.
- Two `15|c` drafts cited "§6.3", which no card supports; they now cite `第 6 章模型 2（[[hart1995firms/15]]）`. One invented forward sentence in its chapter 2 was cut.

## Import pass (all nine)

`imports.mjs plan` proposed no import line for any of the nine. The three false shares (a comparison Link from the Model Card to another model's part) moved into chapter 1 of their model, cue and Link unchanged: `10|a` (`对照 [[5|a|a|7]]`), `11|a|7` (`对应 [[5|a|a|1]]`), `14|a` (`对照 [[11|a|12|f|2]]`). The `false_share` advice is gone in all three.

Left as advice: `imports.mjs plan` reads the moved `对应 [[5|a|a|1]]` line in `11|a|7|b|1` as a possible copy. The chapter does not restate `5|a`'s chapter 1 (it states the Aghion–Bolton E-control result); the line is a correspondence, so nothing changed. `13|a` keeps its migrated `整块 [[11|a|12|f|3]]：共用` import (16 borrowed Cards, nothing in `13|a` rests on them); the import check passes, and whether this share should stay is the owner's decision (the migration of 2026-10-31 kept it).

## Checks after

For each of the nine models:
- `check-model.mjs`: `self_sufficient` true, no closure, forward, reach-in or unanchored fault.
- `check-settings.mjs`: all in the setting part, as a dependency tree.
- `check-network.mjs`: `network: clean`. Advice only: chapters 2 and 3 of `16|a` have no `结果：` card (their sides are derivations and case cards).
- `check-premises.mjs`: no premise used before its card.
- `imports.mjs check`: exit 0.

The three folded models (`5|a`, `11|a|12`, `20|d`) still pass all five checks. `list_broken_links` is empty. The tree holds 1095 Cards (1064 before + 9 notation cards + 22 chapter cards). `model-survey.mjs` lists the same 19 heads.

## Tool fixes (global skills)

- `zt-model-fold/scripts/model-survey.mjs` read the heads with `search_cards` (200-result cap) and listed only 15 of the 19 heads; §13 records the same fault as fixed, but the installed copy had lost the fix. It now reads them from `get_tree` (`modelHeads`), with a test in `model-survey.test.mjs` (3 tests pass).
- `zt-model-restructure/scripts/tree-mover.mjs` saved the index of the move it had just made, so a resume repeated that move (it failed on `16|a` after a stop at move 24 of 50: "move target parent does not exist outside the moved subtree"). It now saves the index of the next move, and the resumed `16|a` run finished all 50 moves. A new test, `tree-mover.test.mjs`, runs the real script in a temp folder beside a fake `zt.mjs` (no live box). The fake fails the third move; the resumed run must then produce the same move log as an uninterrupted run. With the old state line the resume test fails.
- `tree-mover.mjs` speed (after the pass, at the owner's request). A move costs about 2.5 s on the live box, and the old mover made two moves per Card: one to park it, one to place it. It now parks each branch in one move: a Side branch, or a chain under a kept Card, together with its Direct chain, deepest branches first. The parking chain therefore holds the same bare Cards, and placement is unchanged. A failed move is retried once after 3 s, and a second failure stops the run with the move named. A state file from the older mover (no `planner` field) keeps the per-Card parking it started with, so a run in progress resumes safely. `tree-mover.test.mjs` now runs the script against a simulator of zt's moves (four tests: the target tree, the resume, the retry, the older state file). The nine specs of this pass were replayed on their frozen starting trees. The simulator reproduces the 248 moves of the real runs with the old planner. The new planner builds identical final trees with 168 moves, 32% fewer, which saves about 3½ minutes for this book.
- `model-survey.mjs` counted a false share (a Model Card Link to another model's part, which `get_model` lists in `build.false_share`) as a `legacy share`, so it joined models into one candidate group: `1|b` with `5|a`, `12|f` and `14|a` with `11|a|12`. A new `sharedKind` names each shared entry `import`, `false share` or `legacy share`. A false share never joins a group: it is listed under `read`, or named beside a rests-on Link in the `builds on` line. Two tests were added, and all four tests pass. Links without the new `falseShares` field behave as before. On this book the survey now prints `1|b -> 5|a: rests-on 1, false share 1` and `12|f -> 11|a|12: rests-on 1, false share 1`, with 17 candidate groups instead of 14. `1|b` and `12|f` still join through their rests-on Link.
