# Model passes: hansen2021demand (2026-10-31)

Live box (`D:\zt`, service 127.0.0.1:4717). The tree was not locked, and no Card of it was pinned by a locked tree. This record continues §14–14.2 of `docs/model-merge-proposal.md`.

## Models

| Model Card | Level | Own parts | Passes |
|---|---|---|---|
| `10/a` manager-time model | formal | 29+ | done earlier (§14.1–14.2); re-checked clean today |
| `5/a` CBOW embedding | formal | 6 → 8 | settings, restructure, import: done today |
| `6/a` transfer measurement | concept | 4 | skipped: concept model, 4 parts (setting, mechanism, conclusion, example), no derivation |

(The table writes `/` for `|` in addresses.) `6|a` passes `check-model.mjs` (`self_sufficient` true) and `imports.mjs check` as it stands.

## `5|a`: settings pass

Backup: `D:\zt\backups\zt-before-hansen2021demand-settings-5a.sqlite3` (`VACUUM INTO`, integrity ok).

Faults before: no notation card; the setting card had no **起点**. The context definition (window L, the two vectors ρ, α) and the softmax conditional probability, which the card itself calls a modelling choice, sat on the main line as if they were derivations.

Changes:
- New notation card `5|a|a|a` (`符号表与读法`): the symbol and term tables moved out of the setting card, each row linking the card of first use; ℓ(ρ,α) listed under the derivations' symbols.
- The context card moved `alone` to `5|a|a|b` (facet `设定：上下文与两个向量`), and the softmax card to its Direct successor `5|a|a|b|1` (facet `设定：条件概率`). Each has a **起点**.
- The setting card: **起点** `原始设定`, the setting format, text and formula unchanged.

```
5|a|a 设定：词表、one-hot 与低维嵌入     原始设定 (V, v, w_{d,n}, e_v, K)
├ a 符号表与读法
└ b 设定：上下文与两个向量              ← 设定 (L, C(w), ρ_v, α_v)
    b|1 设定：条件概率 (softmax)         ← b
```

## `5|a`: restructure pass

Backup: `D:\zt\backups\zt-before-hansen2021demand-spine-5a.sqlite3`. `tree-mover.mjs` made 8 moves.

```
5|a|a 设定
5|a|a|1 第 1 章：最大化似然，把每个词压成两个低维向量   (new; 依赖 5|a|a|b|1)
└ a 结果：最大化似然并取 L=5、K=200                     (former 估计 card, facet 结果：最大似然估计)
  ├ a 推导：把条件概率代入观测数据的似然                (proof)
  └ b 检验：HBR 嵌入的最相似词是管理术语                 (aside)
```

Text: the same DeepSeek job as the Nobel model (V4.1 Flash, official source, effort high; conversation deleted) drafted the chapter card and rewrote the estimation card in the result format; every fact of the old card is kept, no formula changed. The head's **结构** was rebuilt.

Left as advice: the derivation's line `说明：独立性是写出似然所需的假设，书中没有写出` (check-settings advice, not a fault). It explains a proof step mid-derivation and stays.

## `5|a`: import pass

`imports.mjs check` exits 0; `plan` says `nothing to import`. The model takes nothing from another model.

## Checks after

| Check | `5/a` | `10/a` (re-check) |
|---|---|---|
| `check-settings.mjs` | all in the setting part, as a dependency tree | same |
| `check-model.mjs` | self_sufficient true | true |
| `check-network.mjs` | network: clean (设定 → 1) | clean |
| `check-premises.mjs` | none used before their card | 1 hit: the known pointer from §14.2 |
| `imports.mjs check` | ok | ok |

Card counts: tree 351 → 353 (the notation card and the chapter card); `5|a` own parts 6 → 8. `list_broken_links`: empty before and after.
