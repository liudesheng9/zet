---
name: zt-model-settings
description: "Organize the settings of one ZET model, given the address of its Model Card (for example becker2025division/4|a), so a reader builds the whole setting from scratch: every setting, assumption and symbol in the setting part, arranged as a dependency tree (each setting filed under the setting it builds on, primitives first, the base model's blocks before the model variants, each variant on its own branch), each setting naming its premises in a **起点** section and resting on nothing later, every symbol defined where the reader first meets it; no setting in a chapter. Checked with scripts/check-settings.mjs. Use when the user wants a model's settings, assumptions, notation or symbols organized, gathered, ordered or made readable, or a model's settings checked. Runs after zt-model-fold and before zt-model-restructure."
---

# Organize a model's settings

Input: the **address of the Model Card** (for example `becker2025division/4|a`), written `<head>` below. Output: a setting part that a reader can read top-down and build the whole setting from scratch: each card adds one piece to what the cards above it set up, and every symbol is defined where the reader first meets it.

The order of the model skills: `zt-model-fold` (one Model Card), then this skill (the setting part), then `zt-model-restructure` (the spine of chapters, results and proofs). The restructure keeps the setting part as this skill leaves it. A model that uses another model's building blocks imports them with `zt-model-import` (its **引入** section); do that before this skill, so the imported notation counts.

Tool mechanics (Workspaces, `get_model`, moves, Locks) are in the `zt` skill. A worked record (Becker 2025) is §9.7–§9.8 of `E:\zet\docs\model-merge-proposal.md`.

## The principle: build the setting, do not list it

A pile of settings, each one more Side of the setting card, does not help a reader: they meet symbols before their definitions, cannot tell which assumption a later one changes, and cannot tell the base model from its variants. So the setting part is a **dependency tree**:

- **Primitives first.** The setting card states the agents, preferences, budget and timing; it rests on nothing (`原始设定`). Every other setting builds on settings above it.
- **Each setting is filed under the setting it builds on.** A card filed under another must rest on it; a Direct chain is a run of settings, each building on the one before (task allocation → information → aggregation → wage schedule). A setting that opens a new block (the timing, the draws) is a top-level Side of the setting card.
- **The base model first, then the variants.** The settings every variant shares come first, in blocks (technology, timing and costs, draws). Each model variant gets its own top-level branch, and its sub-variants hang under it (Becker: `设定：封闭经济`; `设定：开放经济` with its benchmark `δ̃=1` and the estimation under it). A variant's **起点** says which variant it is and what it changes (`变体二：开放经济，与封闭经济 […] 并列`).
- **Each setting names its premises.** A `**起点**` section after the one-line summary lists the settings it builds on, one `依赖 [[setting]] 的…：what it uses` line each, or `原始设定` for a primitive. These are the Links the check reads.
- **No premise comes later.** A setting rests only on settings above it. A setting never rests on a chapter: when it seems to, the relation is reversed (the derivation uses the setting), and the result stays in its chapter with the setting pointing to it.
- **Every symbol is defined where the reader first meets it.** The first setting that uses a symbol defines it, in words or by its definition (Becker: the draws card defines the augmented productivity θ̂ and ξ; the information card defines the sensitivity η before the fixed cost uses it). A forward look-up (`$f(\omega)$ 的形式见 […]`) is the exception, for a block overview. The notation card lists every symbol of the model, the derivations' auxiliary symbols in their own section, and no symbol twice with two meanings.
- **Imported blocks keep their symbols at home (#205).** A model that imports a block of another model (its Model Card's **引入** section) does not restate that block's symbols: an imported notation card counts as this model's, and one line `沿用 [[<block>]] 的全部符号` in the setting part binds a whole block's symbols; name a symbol in this model's notation card only when this model renames it (`w 即 [[…]] 的 w_A`). The borrowed Cards are audited in their home model, never here.

And the restriction from before: **the setting part is the only place for settings, and it holds nothing else.**
- Every card whose facet or title says it is a setting or an assumption (`设定…`, `假设…`, `假定…`, `本章设定`, `本阶段的设定`) is in the setting part, and so is the notation card. That includes stage settings, assumed equilibrium conditions, a chapter's technology definitions and the introduction's verbal setting.
- A derivation, a result or a proof step is filed in its chapter, even when it sits under a setting today.
- No chapter states an assumption itself. A chapter card with a `**设定**`, `**假设**` or `**本章设定**` section, a line that opens with `假设`/`假定`, or a symbol table moves that text into a setting card and rests on it. A line that only uses an assumption mid-sentence stays.
- A setting states the assumption only. A sentence in it that uses a result a chapter derives moves to the card that uses the result, and the setting points to the chapter.
- A derivation that starts from a setting names it as a premise: `依赖 [[setting]] 的设定：…`, never `的结果`.

Facets carry the role and the variant, because the GUI shows every Link to a Model part by its facet: `设定：任务配置 (2)`, `设定：第二阶段`, `设定：封闭经济`, `设定：概述`.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive (`D:\zt`, GUI port 4717) only when the user asks for it. Otherwise work on a copied box.
- **Back up first.** `VACUUM INTO` the archive (read-only source; the archive is in WAL mode). Name the backup after the pass, for example `zt-before-<key>-settings.sqlite3`.
- **Locks are the owner's.** A `card_locked` error means: stop, ask the user to unlock the tree in the GUI, and wait.
- **Do not change the mathematics.** Every display formula and equation number survives. Text changes are limited to the `**起点**` section, a definition of a symbol at its first use, a moved sentence, a pointer, a `依赖` line, a facet, and the notation rows.
- **Move cards, never delete a Link.** Scripts move and check; never run an LLM agent per card.

## Step 1: read

- `scripts/check-model.mjs <head>`: confirm `<head>` is the Model Card. A model parted across several Model Cards is folded first (`zt-model-fold`).
- `scripts/check-settings.mjs <head> --symbols`: every fault, each with its fix, and the reading aid.
  - Placement: `outside` (a setting in a chapter, with the derivations under it that stay, `leave`), `inside` (a non-setting in the setting part), `stated` (a chapter that states an assumption), `notation` (a symbol missing from the notation card), `missing` (no notation card of its own, none imported, and no `沿用 … 的全部符号` line).
  - Tree: `start` (a setting with no **起点**), `misfiled` (a setting filed under one it does not build on), `order` (a setting resting on a card a reader meets later).
  - `reads:` lines (with `--symbols`): for each setting in reading order, the notation symbols it uses first. Each must be defined in that card.
- Read every setting card, the cards that rest on it, and the source sections they cite. Write down, for each setting, what it uses and which variant uses it.

## Step 2: design the tree

Draw the target before moving anything:

1. **Blocks of the base model.** Group the shared settings by what they set up (agents; technology and wages; timing and costs; draws). Order the blocks so each uses only blocks above it.
2. **Inside a block, a chain or a branch.** A run where each card uses the one before is a Direct chain; cards that each use the block's head but not each other are its Side branches (the stage settings and the fixed cost under the timing).
3. **The variants.** One top-level branch per variant, after the base blocks; a sub-variant (a special case, the estimation) under its variant.
4. **The premises of each card**, written as its **起点** lines. Check the tree against them: every card rests on the card it is filed under, and on nothing below it.
5. **The symbols.** Walk the tree in reading order; at each symbol's first use, the card defines it. A symbol a card uses before its definition card: move the card after that card, or define the symbol where it is first used.

Becker 2025 after the pass:

```
设定：消费者、工厂、工人与时点        原始设定
├ a 符号表
├ b 设定：概述 (引言)                原始设定
├ c 设定：任务配置 (2)               ← 设定
│   c|1 信息与努力 → c|2 职业加总 (5) → c|3 工资表 (7)
├ d 设定：三阶段时序                 ← 设定, 任务配置
│   d|1 进入条件;  d|a 固定成本;  d|b 第二阶段;  d|c 第三阶段
├ e 设定：三元正态抽签                ← 进入条件, 固定成本 (θ̂, ξ defined here)
├ f 设定：封闭经济                   变体一 ← 进入条件, 抽签
└ g 设定：开放经济                   变体二 ← 时序, 进入条件, 固定成本, 抽签
    g|a 基准开放经济 (δ̃=1);  g|b 估计的数据
```

## Step 3: move and write

Write the moves as one script (`scripts/zt.mjs` `move`), finding each card by its title before each move:

1. File each `leave` derivation in its chapter first (`before` the head of the proof chain that uses it).
2. **Park every setting leaf-first** under the last chapter card (a Direct chain's last step first, then the step before it, then the head), so no move renames another card.
3. **Place top-down** into the designed slots: a block head as the next Side of the setting card, a chain step as the next Direct slot, a branch as the next Side of its parent. Side slots take only the next free letter, which is why everything is parked first.
4. Repack each chapter a setting left: park its later Side branches and bring them back from `a`.

Then the text, in one batch (`zt-digest call <edits> -Allow <addresses>`): the **起点** of every setting (the setting card included); each symbol's definition at its first use; the facets; every `依赖 [[setting]] 的结果` as `的设定`; moved sentences and pointers; the notation rows. A batch runs every call even after one fails, so read the cards back, or rerun the check, before you go on.

## Step 4: verify and record

- `scripts/check-settings.mjs <head> --symbols` prints `settings: all in the setting part, as a dependency tree`; read the `reads:` lines once more against the cards.
- `zt-model-restructure`'s `check-model.mjs` (`self_sufficient` true), `check-network.mjs` (`network: clean`) and `check-premises.mjs` (only pointers and roadmaps; no setting names a later result).
- `list_broken_links` is empty; the tree has the same number of Cards.
- Rebuild the head's **结构** section, record the pass in the project docs, and tell the user which tree is still unlocked.

## Helper facts (zt-digest call)

- Input is a JSON **array** of `{tool, arguments}`; one call per line of output: `N ok <address>`, `N error …`, or `N {json}` for reads.
- With `-Allow <file>` only those Cards may change, and moves are refused. Run moves **without** `-Allow`.
- `edit_card` takes `facet` and `title` with the `body`.
- `move_card` with `before` puts the moved card in that place and makes the displaced card its Direct successor: right for a new first step of a chain, wrong for reordering Side branches.
- The build order is depth-first: a card, its Direct chain (each step with its own Sides), then its Side branches.
- In the Bash tool, heredocs lose backslashes; write scripts and patches with the Write tool.
