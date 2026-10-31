# Model passes: acemoglu2025nobel (2026-10-31)

Live box (`D:\zt`, service 127.0.0.1:4717). The tree was not locked, and no Card of it was pinned by a locked tree. This record continues §15–15.2 of `docs/model-merge-proposal.md`.

## Models

| Model Card | Level | Own parts | Passes |
|---|---|---|---|
| `4/a` micro-foundation model | formal | 33 | done earlier (§15.1–15.2); re-checked clean today |
| `5/a` political losers (§II.D) | formal | 11 → 13 | settings, restructure, import: done today |
| `1/d` colonial origins | concept | 4 | skipped: concept model, 4 parts, verbal chain with no equations |
| `1/e` political institutions and resource distribution | concept | 3 (+3 imported) | skipped: concept model, 3 own parts, arrow diagram and text only |
| `6/a` critical junctures, four frontiers | concept | 3 | skipped: concept model, 3 parts, Figure 11 and text only |
| `7/a` digital inequality | concept | 3 | skipped: concept model, 3 parts, Figure 12 and text only |

(The table writes `/` for `|` in addresses.) Each skipped model passes `check-model.mjs` (`self_sufficient` true) and `imports.mjs check` as it stands.

## `5|a`: settings pass

Backup: `D:\zt\backups\zt-before-acemoglu2025nobel-settings-5a.sqlite3` (`VACUUM INTO`, integrity ok).

Faults before: no notation card; the setting card had no **起点**; the assumption card (`假设：失权概率 p、贴现因子 δ 与支付次序`) was the first step of the main line.

Changes:
- New notation card `5|a|a|a` (`符号表与读法`): the symbol and term tables moved out of the setting card, each row linking the card where the symbol first appears; μ, α, A(α) and the frontier point to `4|a`'s notation card `4|a|a|a`.
- The assumption card moved `alone` to `5|a|a|b`, facet `设定：失权与贴现`, title `设定：…`. It now defines u_r^A, u_r^B, u_r^C at first use and has a **起点** on the setting card.
- The setting card got the setting format: **起点** `依赖 [[4|a|a|1]]` (the imported frontier framework and its points A, B, C), **内容**, **为什么需要它**. The text and Figure 9 are unchanged.

```
5|a|a 设定：精英从 C 点出发          ← 4|a|a|1 (imported)
├ a 符号表与读法
└ b 设定：失权与贴现                 ← 设定 (u_r^{A,B,C}, p, δ)
```

## `5|a`: restructure pass

Backup: `D:\zt\backups\zt-before-acemoglu2025nobel-spine-5a.sqlite3`.

The model has a single mechanism, so the spine has one chapter. `tree-mover.mjs` made 20 moves.

```
5|a|a 设定
5|a|a|1 第 1 章：失权风险使有耐心的精英抵制有益技术   (new; 依赖 5|a|a|b)
├ a 结果：δ 超过 δ* 时精英宁愿留在 C 点               (former 命题 card, facet 结果：政治输家)
│ ├ a 推导：抵制的现值 → a|1 采用的现值 → a|2 比较得 δ*   (proof chain, 3 cards)
│ ├ b 补：δ* 落在 0 与 1 之间的条件
│ └ c 例子：δ*=2/3
├ b 对应：同一逻辑使低效制度延续
├ c 变体：精英选更小但更安全的外移
└ d δ 越大精英越保守
```

Text: one DeepSeek job (V4.1 Flash, official source, effort high; conversation deleted) drafted the chapter card and rewrote the result card in the result format. Two repairs by hand: the chapter's aside list named the wrong cards (fixed to `1|b`, `1|c`, `1|d`), and the result card's sentences on footnote 17 and the chart caption, which the draft dropped, were restored. A script confirmed the δ* display formula and the chart block are byte-identical to the original. The head's **结构** was rebuilt (two levels), and the import line `整块 [[4|a|a|1]]：共用` now says what the model uses (the frontier and the points A, B, C).

One network fault after the moves: the notation row for u_r^{A'}, p' read "路径依赖变体中…", so the word 依赖 made the row a premise on the aside `1|c`. The row is a look-up pointer, so its description was reworded to name the variant by its title.

## `5|a`: import pass

`imports.mjs check` exits 0; `plan` says `nothing to import`. The two existing imports (整块 `4|a|a|7|b`, 整块 `4|a|a|1`) are kept. The earlier copy advice (setting card corresponds to `4|a|a|1`) is resolved: the setting card does not restate the framework; it rests on it by its **起点** line.

## Checks after

| Check | `5/a` | `4/a` (re-check) |
|---|---|---|
| `check-settings.mjs` | all in the setting part, as a dependency tree | same |
| `check-model.mjs` | self_sufficient true | true |
| `check-network.mjs` | network: clean (共用 ×2 → 设定 → 1) | clean |
| `check-premises.mjs` | none used before their card | none |
| `imports.mjs check` | ok | ok |

Card counts: tree 360 → 362 (the notation card and the chapter card); `5|a` own parts 11 → 13. `list_broken_links`: empty before and after.
