# Model board in blocks: a comprehensible model network

Status: proposal (2026-10-28), not implemented. Owner decision: blocks top to bottom (§8.1); decisions 2–4 open, with the proposed defaults. Scope: the GUI's **model board** (`modelBoard` in `src/gui/app.js`, design in [model-structure-design.md](model-structure-design.md) "The model board"), the model data the GUI reads (`get_model` / the card summary), and the Map's view of a model. It changes no Card and no archive rule.

## 1. Problem

The owner, looking at the Becker model (`becker2025division/4|a`, 142 parts) after it was rebuilt as an intuition spine (setting, 11 chapters, results, proofs; [model-merge-proposal.md](model-merge-proposal.md) §9.3): the model network is still not comprehensible, and it should show **range blocks** ("this block is the first part, that block is the second part") with the **Links in and out between the blocks**.

What the board does today, measured on that model:

| Board rule today | Effect on a 142-part model |
|---|---|
| Rows by rests-on depth; a part sits under the mean of what it rests on | Cards of different chapters share a row; the chapters the Cards were filed in are invisible. |
| One thread per relation, every kind | 446 relations (240 rests-on, 157 refers, 5 illustrates, 3 contrasts) give **1,215 SVG paths** on the board; most are dashed `REFERS TO` threads that cross the whole panel. |
| Every part is drawn (pale when far from the open part) | 142 slips at once; the reader cannot tell which are chapters, results, or proof steps. |
| Derivation parts as slim slips in the same rows | 72 proof steps compete for space with the 11 chapter cards that carry the meaning. |

The board was designed for models of 5–20 parts. Its rules do not use the structure the model now has: the main line (chapters), the Side branches (results), and the proof chains under them.

## 2. Principles

The owner's GUI principles hold for every proposal below:

1. **Working memory and attention are limited.** The view shows a few blocks at full ink and folds the rest; detail appears only where the reader is looking (the attention-first layout of the Map, commit f9dea67, and the comprehension principle of the model work).
2. **Building a set of related Cards is easy** and the set takes the most emphasized place.
3. **Physical mapping.** Every element is a desk object: a block is a **card tray with a divider tab**, a fold is a **stack** with its count, a Link bundle is a **thread** (or a ribbon of threads), and motion follows physical behaviour.
4. **Fast and deterministic.** Linear-time layout, no force simulation; the same model always looks the same.

## 3. Proposals

### P1. Blocks: one tray per chapter, ranges of chapters as parts

A **block** is a main-line card with everything filed under it: a chapter card, its results, its proofs, its asides. The setting with the notation card and the settings is block 0. Blocks come from the model's own structure, so no new data is needed for a model built as a spine.

A **range block** (a part, 第一部分 …) groups consecutive chapters. Becker would read:

```
┌ 第一部分 构件 ───────┐┌ 第二部分 求解 ──────┐┌ 第三部分 三种均衡 ────────┐┌ 第四部分 量化 ┐
│ 设定 · 1 直觉 ·      ││ 4 第三阶段 ·        ││ 7 封闭经济 · 8 开放经济 · ││ 10 量化 ·     │
│ 2 生产 · 3 工资      ││ 5 第二阶段 · 6 第一 ││ 9 基准开放经济            ││ 11 主要结论   │
└──────────────────────┘└─────────────────────┘└───────────────────────────┘└───────────────┘
```

Where the ranges come from, in this order:

1. **Authored ranges** in the Model Card: a `**分部**` section in the head's body that names each part and its first chapter (`- 第一部分 构件：[[设定]] 起`). The author (or the architect, §7) decides what belongs together.
2. **Default:** without authored ranges, each chapter is its own block and there is no part level.
3. **Old models** without a main line of chapters: a block per section anchor the parts were digested from (the group), so even a model that was never restructured shows its source sections.

### P2. The block overview: the default board

The board opens on the **block overview**, not on the parts:

```
 ┃ 第一部分 构件
 ┃ ┌──────────────────────────────────────────┐
 ┃ │▌第 2 章 生产技术   结果 1 · ▤ 证明 6     │ ─┐
 ┃ └──────────────────────────────────────────┘  │ 依赖 ×9
 ┃ 第二部分 求解                                   │
 ┃ ┌──────────────────────────────────────────┐  │
 ┃ │▌第 5 章 第二阶段   结果 3 · ▤ 证明 13    │ ◀┘ ─┐
 ┃ └──────────────────────────────────────────┘     │ 依赖 ×14
 ┃ 第三部分 三种均衡                                  │
 ┃ ┌──────────────────────────────────────────┐     │
 ┃ │▌第 7 章 封闭经济   结果 2 · ▤ 证明 8     │ ◀───┘
 ┃ └──────────────────────────────────────────┘
```

- Each block is drawn as a **tray**: its chapter card as a full index card on the left of the tray (title and the first sentence of 直觉), its results as tabs beside it (`结果：最优性`, `结果：增广生产率`, …), and its proofs as one stack with a count (`证明 13 步`).
- **Blocks lie top to bottom in one column, in reading order** (owner decision, 2026-10-28), like the Map's main line. A range block is a bar down the left edge with the part's name on its divider tab, spanning its chapters. Ribbons between blocks run in a gutter on the right side, so they never cross a tray; a ribbon that skips blocks runs further out in the gutter.
- At most about 12 trays are inked at once: the open block, its neighbours, and the blocks it links to; the rest shrink to their divider tabs (the existing `here`/`near`/`mid`/`far` levels, applied to blocks).

### P3. Links between blocks: bundled, directed, counted

The Links between Cards of different blocks are the "links in and out" the owner asked for. Drawn one by one they are the 1,215-path tangle; drawn **per pair of blocks** they are readable:

- **One ribbon per ordered pair of blocks**, from the block rested on to the block that rests on it, with its width set by the count and a label `依赖 ×14`. Kinds other than rests-on (contrasts, relaxes, illustrates, corresponds) get their own thin ribbon in their own colour and word.
- **Plain references (`refers`) are not drawn by default.** They sit behind a switch, like the `Term uses` switch, because a reference is navigation, not reasoning. On Becker this removes 157 of the 446 relations before anything else.
- **Hovering a ribbon** lists its Card pairs (`第 5 章 · 结果：增广生产率 ← 第 7 章 · 推导：边际工厂的收入方程`), each row opening that Card; clicking a ribbon opens both blocks side by side with only those threads drawn.
- **Ports on the tray edge.** Each Card that has Links leaving its block shows a small port on the tray's edge, `→ 第 7 章 ×3` / `← 第 2 章 ×2`, so the reader sees in and out without a line across the screen. On the open block the ports are always visible; on others, only on hover.
- **Checks shown on the ribbons.** A ribbon that runs backwards (a block resting on a later block) is drawn in the fault colour with the `forward` fix from `build`; a block with no ribbon from earlier blocks shows the `unanchored` mark. The board then explains the build faults in the reader's terms.

### P4. Opening a block: results in a row, proofs as stacks

Clicking a tray (or pressing Enter on it) **opens the block** in place; the other trays slide aside to their tabs (focus + context, with the existing lift and settle motion):

```
 ┌ 第 5 章：工厂选职业数量，使边际收入等于管理跨度成本 ───────────────────────────┐
 │ 直觉：多一个职业，边际成本降一点，管理跨度成本升一点……                         │
 │                                                                              │
 │  结果：最优性            结果：增广生产率          结果：临界值与条件            │
 │  ┌──────────┐            ┌──────────┐              ┌──────────┐              │
 │  │ (14) …   │            │ (15) …   │              │ (16)(17) │              │
 │  └──────────┘            └──────────┘              └──────────┘              │
 │  ▤ 证明 5 步              ▤ 证明 5 步                ▤ 证明 3 步                │
 │                                                                              │
 │ ← 第 4 章 ×3 (价格 (12))                      → 第 6 章 ×4 · → 第 7 章 ×6     │
 └──────────────────────────────────────────────────────────────────────────────┘
```

- The chapter card on top, its results as a row of cards, each with its proof as a **stack** (`▤ 证明 5 步`).
- Clicking a stack **fans** it into the proof chain: a vertical run of slim cards with their `目的：` line, the threads between consecutive steps only, and the result card at the end. One stack is fanned at a time.
- Threads are drawn only inside the open block and only for the open card (what it rests on, what rests on it). Links leaving the block end at the port on the tray edge, labelled with the target block.

### P5. The reading line

A thin **reading line** along the left edge of the board shows the whole model as its divider tabs in order (`设定 · 1 · 2 · 3 | 4 · 5 · 6 | 7 · 8 · 9 | 10 · 11`, the bars marking the parts), with the open block raised. Arrow keys walk it: `↑`/`↓` move to the previous or next block, `Enter` or `→` opens the block, `←` closes it. The reader can always see where they are and how much is left, which is the cheapest aid to working memory.

### P6. The same blocks on the Map

The Map's focus lens already folds a model's Side branches into `+N` stacks when the pointer is on the main line (§9.3 of the merge proposal). Two small changes make it agree with the board:

- Inside a model, the lens shows the **range-block divider** (`第二部分 求解`) as a label on the main line where a part begins.
- A Model Card's own Links to all its parts (the **结构** list) are left out of the Map's pointer threads, as Term uses are: they are the model's table of contents, not reasoning, and they are what makes the head's map neighbourhood a fan of dashed threads to every chapter and result today.

### P7. Data from the backend, layout in the front end

So that the front end stays linear and simple, `get_model` (and the card summary's `model`) gains:

```json
"blocks": [
  {"id": "b5", "head": "Key/4|a|a|5", "part": "第二部分 求解", "title": "…", "facet": "第 5 章：第二阶段",
   "results": [{"address": "…", "facet": "结果：最优性", "proof": {"first": "…", "steps": 5}}],
   "members": 22}
],
"block_edges": [
  {"from": "b4", "to": "b5", "kind": "rests-on", "count": 3,
   "pairs": [{"from": "Key/4|a|a|5|b|a", "to": "Key/4|a|a|4|c"}]}
]
```

- Blocks are computed from the build order (`line` = `main` starts a block; `side` and the parts filed under it belong to it). Range blocks come from the head's `**分部**` section when it is present.
- `block_edges` aggregates the existing relations by block and kind; the pairs list the Cards (for hover and the two-block view).
- Everything is one pass over the parts and one over the relations.

## 4. What this removes and keeps

- **Removed by default:** the depth rows for a model with more than about 20 parts (they stay for small models, where they work), the refers threads, the head's 结构 fan on the Map, every proof slip that the reader has not asked to see.
- **Kept:** threads with reasons, the lift-and-settle motion, divider tabs, attention levels, the loose tray, the build faults with their fixes, the `Term uses` switch.

## 5. Acceptance

On the Becker model:

1. The board opens on the block overview: 12 blocks in 4 parts (with authored ranges) or 12 blocks without; at most 12 trays inked; no Card slip outside the open block.
2. At most one ribbon per ordered pair of blocks and kind; with refers off, the overview draws **fewer than 60 paths** (today 1,215).
3. Opening a block shows its chapter card, its results, and its proofs as stacks; fanning one proof shows only that chain's threads.
4. Every Link that leaves the open block is visible as a port with its target block and count; hovering lists the Card pairs.
5. A backwards ribbon or an unanchored block shows the build fault and its fix.
6. Layout time is linear in parts plus relations (a Node test on a 500-part synthetic model under 50 ms); the same model always lays out the same.
7. A model of 5–20 parts without chapters still shows today's depth-row board.

## 6. Slices

1. **B1 backend:** `blocks` and `block_edges` in `get_model` and the card summary, with tests on a spine fixture and on an old flat model (group fallback).
2. **B2 overview:** trays, divider tabs, results as tabs, proof stacks with counts, the reading line; attention levels per block.
3. **B3 ribbons and ports:** bundled inter-block threads with counts and kinds, the refers switch, hover lists, the two-block view, fault colouring.
4. **B4 open block:** results in a row, fanned proofs, threads only inside the block.
5. **B5 Map:** range-block labels on the main line, the head's 结构 Links left out of the pointer threads.
6. **B6 authored ranges:** the `**分部**` section in the head, written by hand or by the model architect (§7); the restructure skill (`zt-model-restructure`) and its card formats add it.

## 7. The model compiler

The architect already plans each model; with this proposal its plan also names the parts (range blocks) and the chapters in each, and the compiler writes the head's `**分部**` section. The restructure skill adds the section for models it rebuilds. A model without it still gets one block per chapter.

## 8. Open decisions for the owner

1. **Orientation: decided (2026-10-28).** Top to bottom in one column, like the Map's main line; ribbons in a gutter on the right.
2. **Range blocks:** authored by hand in the head's `**分部**` section only, or also proposed by the architect for every new model?
3. **Refers:** hidden behind a switch (proposed), or dropped from the board entirely?
4. **Old models:** fall back to blocks by source section (proposed), or keep today's depth rows for them?

