---
name: zt-model-settings
description: "Organize the settings of one ZET model, given the address of its Model Card (for example becker2025division/4|a), so a reader builds the whole setting from scratch: every setting, assumption and symbol in the setting part, arranged as a dependency tree (each setting filed under the setting it builds on, primitives first, the base model's blocks before the model variants, each variant on its own branch), each setting naming its premises in a **起点** section and resting on nothing later, every symbol defined where the reader first meets it; no setting in a chapter. Strict placement (the setting card is the Model Card's first Side successor, the notation card the setting card's, every setting in the setting card's Side branches); one card format (**起点**, **设定** or **假设**, **说明**); the notation card covers every symbol of every setting and lists only symbols the model uses, each with the card where a reader meets it first; an assumption is stated once and cited by its Link everywhere; statements reallocated so each card sets up one balanced piece. Checked with scripts/check-settings.mjs. Use when the user wants a model's settings, assumptions, notation or symbols organized, gathered, ordered or made readable, or a model's settings checked. Runs after zt-model-fold and before zt-model-restructure."
---

# Organize a model's settings

Input: the **address of the Model Card** (for example `becker2025division/4|a`), written `<head>` below. Output: a setting part that a reader can read top-down and build the whole setting from scratch: each card adds one piece to what the cards above it set up, and every symbol is defined where the reader first meets it.

The order of the model skills: `zt-model-fold` (one Model Card), then this skill (the setting part), then `zt-model-restructure` (the spine of chapters, results and proofs). The restructure keeps the setting part as this skill leaves it. A model that uses another model's building blocks imports them with `zt-model-import` (its **引入** section); that skill runs after `zt-model-restructure`, as the last model pass: it writes the import lines, settles imported symbols in the notation card (its step 6, then `check-settings.mjs` again), and refreshes the explanations.

Tool mechanics (Workspaces, `get_model`, moves, Locks) are in the `zt` skill. Worked records: Becker 2025 (§9.7–§9.8) and the strict pass on Acemoglu 2007 (§11.3) of `E:\zet\docs\model-merge-proposal.md`.

## The place

The structure is fixed, and the check refuses any other (`place`, `outside`, `inside`):

- The **setting card** is the Model Card's first Side successor, `<head>|a`. It gathers the settings of every source of the model. The chapters are its Direct successors.
- The **notation card** (facet `符号表与读法`) is the setting card's first Side successor, `<head>|a|a`.
- **Every setting, assumption and definition of the model is in the setting card's Side branches**, and nothing else is.

## An empirical model: the data tree and the specification ladder

A Model Card of level `empirical` (level 3, docs/empirical-models-proposal.md §9.2) has no 设定 card: its setting part is the **data tree** and the **specification ladder**, and its parts carry the roles of their facet prefixes (`get_model` names each part's `role`). The structure is fixed, and `scripts/check-settings.mjs <head>` checks it when the model is empirical:

- The **data card** (facet `数据`) is the Model Card's first Side successor, `<head>|a`, and the first main-line part. The **变量表** is the data card's first Side successor (`place`): one row per variable and symbol of every specification (`| 变量 | 含义 | 度量 | 来源 | 首次出现 |`). Every symbol of a specification's equation has a row (`uncovered`), and no row is unused (`unused`). Samples (`样本：`), measures (`测量：`) and descriptive facts (`描述：`) hang under the data card, each with its **起点** (`start`), and none rests on a card of the data tree a reader meets later (`order`: reorder the data card's Side branches).
- The **design card** (`设计`) holds the identifying assumptions as numbered cards (`假设 1`, `假设 2`), each filed as a Side successor of the design card itself, never chained under another assumption (`assumption`). Every test cites its assumption by Link (`依赖 [[假设 1]]`), never by name.
- `规格：基准` stands on the main line before every variant. Each variant hangs under the specification it changes, rests on it, and states only its change in **相对基准的改动** (`variant_change`). No specification is filed in a chapter (`spec_in_chapter`): a robustness chapter holds the estimates of the variants, never the variants.
- The build report's role faults (`empirical_shape`, `estimate_unbound`, `spec_unbound`, `test_unbound`) are listed too; each is fixed by moving a card or adding the rests-on line the role needs.

The formal rules below (one notation card, **起点**/**设定**/**说明**, assumptions in the setting part) apply to a formal model; for an empirical model the rules of this section replace them.

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

## One card format

Every setting card has the same three sections, in this order (`format`):

```
**起点**
- 依赖 [[setting]] 的设定：what it uses.      (or: 原始设定：…; a variant: 变体二：…，与 […] 并列；与基准相比…)

**设定**                                        (an assumption card: **假设**)
1. One statement per item: an agent, a timing, a choice, a definition.
2. A display formula follows the item that introduces it, with its original number:

$$… \tag{2}$$

**说明**                                        (optional)
Remarks, the role of the setting, pointers to later cards.
```

- The title says the kind: `设定：…` for a setting, `假设 <name>：…` for an assumption (`假设 1：…`, `假设 A1：…`). The facet starts with `设定` or `假设`.
- Premises appear only in **起点**: a `依赖 [[…]]` line anywhere else moves there.
- No symbol table and no term table in a setting card: their rows go to the notation card.
- **Assumptions look alike.** An assumption with branches says `<function> 满足下面两个分支之一。` and lists `1. 分支一（凹）：…`, `2. 分支二（不变规模报酬）：…`; its role (what it guarantees, which variants use it) goes to **说明**.

## The notation card

The notation card is the one place for symbols and terms, and the check reads it row by row:

- **It covers every setting** (`uncovered`): every letter a setting uses in a formula, outside sub- and superscripts, has a row, including the indices (`$i$` 企业, `$j$` 分量, `$s$` 生产者) and a bound variable such as $\epsilon$. A name that is not a symbol (经济 D) is written outside math.
- **It lists only symbols the model uses** (`unused`): a row whose symbol no card uses is dropped or rewritten as the cards write it.
- **Each row names the card where a reader meets the symbol first** (`first`), in the reading order: the setting part, then the chapters. That card defines it. One letter with two meanings in the source gets one row that names both (`$\Pi(\theta),\Pi$ | 经济 C 的目标 (7)；经济 M 的利润 (12)`).
- Rows are grouped by block, in the order of the setting tree; the derivations' auxiliary symbols come last, then the term table (`术语 | 不用`).

## Assumptions: stated once, cited by Link

- **One statement, one card** (`repeat`). A variant that keeps an assumption and changes only its object states the change (`假设 2 的两个分支不变，作用对象换为 (14) 的子成分 G`), never the branches again. Two assumptions with different names (a weakening) may run parallel.
- **Every card that names an assumption links its card** (`cite`): `设[[…|c|a]]成立`, not `设假设 1 成立`. The GUI shows a Link to a model part by its facet, so the facet `假设 1` reads as 假设 1. A `[[label@address]]` Link is for Term Cards only.
- Watch the relation words: `放宽`/`推广` before a Link make a `relaxes` relation from this card to the linked one. Write `只要求较弱的 [[假设 2]]`, not `放宽假设 1，只用 [[假设 2]]`.

## Balance

The check gives advice (`balance`), not faults: a setting with more than ten items is split; a setting (not an assumption) with one item and no display formula joins the setting it builds on; a card with more than seven settings filed under it groups them in blocks. Reallocate statements so each card sets up one piece: a statement belongs to the first card whose subject it is (the price vector of the common environment in the setting card, `技术由每家企业分别决定` in the base economy, the choice sets $\mathcal Z,\mathcal L$ where the reader first meets them), and a later card that repeats it drops it or points back.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive (`D:\zt`, GUI port 4717) only when the user asks for it. Otherwise work on a copied box.
- **Back up first.** `VACUUM INTO` the archive (read-only source; the archive is in WAL mode). Name the backup after the pass, for example `zt-before-<key>-settings.sqlite3`.
- **Locks are the owner's.** A `card_locked` error means: stop, ask the user to unlock the tree in the GUI, and wait.
- **Do not change the mathematics.** Every display formula and equation number survives byte for byte: compare the display formulas of the old and the new bodies before you write (a script, not by eye). Text changes are limited to the `**起点**` section, the format (sections, numbered items, titles), a definition of a symbol at its first use, a moved sentence, a restated sentence dropped, a pointer, a Link for a named assumption, a `依赖` line, a facet, and the notation rows.
- **Move cards, never delete a Link.** Scripts move and check; never run an LLM agent per card.

## Step 1: read

- `scripts/check-model.mjs <head>`: confirm `<head>` is the Model Card. A model parted across several Model Cards is folded first (`zt-model-fold`).
- `scripts/check-settings.mjs <head> --symbols`: every fault, each with its fix, and the reading aid.
  - Placement: `place` (the setting card is not `<head>|a`, or the notation card not its `|a`), `outside` (a setting in a chapter, with the derivations under it that stay, `leave`), `inside` (a non-setting in the setting part), `stated` (a chapter that states an assumption), `notation` (a symbol a card introduces with 记 that the notation card lacks), `missing` (no notation card of its own, none imported, and no `沿用 … 的全部符号` line).
  - Tree: `start` (a setting with no **起点**), `misfiled` (a setting filed under one it does not build on), `order` (a setting resting on a card a reader meets later).
  - Symbols: `uncovered` (a setting uses a letter the notation card lacks), `unused` (a notation row no card uses), `first` (a row names the wrong first card).
  - Cards: `format` (`title`, `section`, `depends`, `table`), `repeat` (a setting restates an earlier one), `cite` (a card names an assumption without its Link).
  - Advice: `balance` (heavy, light, wide), and `uses` (a line that uses an assumption mid-sentence; it normally stays).
  - `reads:` lines (with `--symbols`): for each setting in reading order, the notation symbols it uses first. Each must be defined in that card.
- Read every setting card, the cards that rest on it, and the source sections they cite. Write down, for each setting, what it uses and which variant uses it.

## Step 2: design the tree

Draw the target before moving anything:

1. **Blocks of the base model.** Group the shared settings by what they set up (agents; technology and wages; timing and costs; draws). Order the blocks so each uses only blocks above it.
2. **Inside a block, a chain or a branch.** A run where each card uses the one before is a Direct chain; cards that each use the block's head but not each other are its Side branches (the stage settings and the fixed cost under the timing).
3. **The variants.** One top-level branch per variant, after the base blocks; a sub-variant (a special case, the estimation) under its variant.
4. **The premises of each card**, written as its **起点** lines. Check the tree against them: every card rests on the card it is filed under, and on nothing below it.
5. **The symbols.** Walk the tree in reading order; at each symbol's first use, the card defines it. A symbol a card uses before its definition card: move the card after that card, or define the symbol where it is first used.
6. **The content of each card.** List each card's statements and give each to the card whose subject it is (see Balance). Shared building blocks used by every variant (the net production function and the assumptions on it) are a block of their own before the variants, not a step inside one variant's chain. A sub-variant that extends a variant (oligopoly after monopoly) hangs under it.

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

Write the moves as one batch (`scripts/zt.mjs` `tidy`): `move_span` with `side_of` or `after` for each card, then one `reorder` of the setting card's Sides; every address names the card at batch start, and the preview shows the `moved` map before you commit. The older way, one move at a time (`move`), finds each card by its title before each move:

1. File each `leave` derivation in its chapter first (`before` the head of the proof chain that uses it).
2. **Park every setting leaf-first** under the last chapter card (a Direct chain's last step first, then the step before it, then the head), so no move renames another card.
3. **Place top-down** into the designed slots: a block head as the next Side of the setting card, a chain step as the next Direct slot, a branch as the next Side of its parent. Side slots take only the next free letter, which is why everything is parked first.
4. Repack each chapter a setting left: park its later Side branches and bring them back from `a`.

Then the text, in one batch (`zt-digest call <edits> -Allow <addresses>`): every setting in the card format (the setting card included), with its **起点**, titles and facets; each symbol's definition at its first use; every `依赖 [[setting]] 的结果` as `的设定`; moved sentences, dropped restatements and pointers; the whole notation card (rows by block, indices, first cards, terms). Write the edits as a script whose bodies are `String.raw` templates, and compare the display formulas before writing. A batch runs every call even after one fails, so read the cards back, or rerun the check, before you go on.

Last, the citations: replace each bare `假设 N` the `cite` faults name with `[[<its card>]]` (a script over `check-settings.mjs --json`; `假设 1 或 2` names two cards), and point the `first` rows to the cards the check names.

## Step 4: verify and record

- `scripts/check-settings.mjs <head> --symbols` prints `settings: all in the setting part, as a dependency tree`; read the `reads:` lines once more against the cards.
- `zt-model-restructure`'s `check-model.mjs` (`self_sufficient` true, and no new `relaxes`, `objects` or other relation your wording made), `check-network.mjs` (`network: clean`) and `check-premises.mjs` (only pointers and roadmaps; no setting names a later result).
- `list_broken_links` is empty; the tree has the same number of Cards.
- Rebuild the head's **结构** section, record the pass in the project docs, and tell the user which tree is still unlocked.

**Relink the Terms last.** A rewritten Card loses Term links, and nothing else restores them. Run `pwsh -NoProfile -File E:\zet\scripts\book-digest\term-relink.ps1 -CitationKey <key>`. It keeps every visible character, and it fails if a second plan still changes a Card.

## Helper facts (zt-digest call)

- Input is a JSON **array** of `{tool, arguments}`; one call per line of output: `N ok <address>`, `N error …`, or `N {json}` for reads.
- With `-Allow <file>` only those Cards and the Cards the run makes may change; the server refuses moves, inserts, deletes and a `create_successor` with `before` (`scope_refused`). Run moves **without** `-Allow`.
- `edit_card` takes `facet` and `title` with the `body`.
- `move_card` with `before` puts the moved card in that place and makes the displaced card its Direct successor: right for a new first step of a chain, wrong for reordering Side branches.
- The build order is depth-first: a card, its Direct chain (each step with its own Sides), then its Side branches.
- In the Bash tool, heredocs lose backslashes; write scripts and patches with the Write tool.
