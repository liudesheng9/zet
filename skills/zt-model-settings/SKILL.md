---
name: zt-model-settings
description: "Organize the settings of one ZET model, given the address of its Model Card (for example becker2025division/4|a): every setting, every assumption and every symbol of the model goes into the setting part (the setting card and its Side branches, with one notation card), and nothing else stays there; no chapter keeps a setting, a stage setting, an assumption of its own or a local symbol table. Checked with scripts/check-settings.mjs, which refuses any setting outside the setting part. Use when the user wants a model's settings, assumptions, notation or symbols organized, gathered or moved into the setting part, or a model's settings checked. Runs after zt-model-fold and before zt-model-restructure."
---

# Organize a model's settings

Input: the **address of the Model Card** (for example `becker2025division/4|a`), written `<head>` below. Output: one setting part that holds every setting, every assumption and every symbol of the model, and a model whose chapters hold none.

The order of the model skills: `zt-model-fold` (one Model Card), then this skill (the setting part), then `zt-model-restructure` (the spine of chapters, results and proofs). The restructure keeps the setting part as this skill leaves it.

Tool mechanics (Workspaces, `get_model`, moves, Locks) are in the `zt` skill. A worked record (Becker 2025: six settings out of four chapters) is §9.7 of `E:\zet\docs\model-merge-proposal.md`.

## The rule

**The setting part is the only place for settings, and it holds nothing else.** It is restricted: `scripts/check-settings.mjs` fails on any exception.

- **The setting part** is the setting card (`get_model` names it `build.setting`, the first card of the main line) and everything filed in its Side branches. The chapters are its Direct successors.
- **What goes there:** every card whose facet or title says it is a setting or an assumption (`设定…`, `假设…`, `假定…`, `本章设定`, `本阶段的设定`), and the notation card (`符号表与读法`). Stage settings of a backward induction (`设定：第二阶段只选职业数量`), equilibrium conditions the book assumes (`设定：自由进入使总利润为零`), the technology and aggregation definitions of a chapter, and the verbal setting of an introduction all count.
- **What does not:** a derivation, a result, a proof step. A card the book derives is filed in its chapter, even when it sits under a setting today (Becker: `利润 (11)` was filed under the third-stage setting; it now heads the proof chain of chapter 4's price result).
- **No chapter states an assumption itself.** A chapter card with a `**设定**`, `**假设**` or `**本章设定**` section, a line that opens with `假设`/`假定`, or a symbol table moves that text into a setting card and rests on it (`依赖 [[setting]]`). A line that only uses an assumption mid-sentence (`由于假设 γ>σ-1，指数为正`) stays; the check lists it as advice.
- **Every symbol is in the notation card.** A symbol a derivation introduces (`记 $H=…$`, `记号：$C(\omega)=…$`) gets a row there, in a section for the auxiliary symbols, naming the chapter that uses it. The derivation still restates it at first use.
- **A setting is a primitive.** It states the assumption only and rests on nothing derived later. When a setting moved into the setting part names a result a chapter derives (`第三阶段的价格 (12) 已经解出`, `约束：需求 (1)、生产 (6)`), that sentence is a use of the result: move it to the card that uses the result (the derivation that starts from the setting), and let the setting point to the chapter (`工厂已知第三阶段的解（第 4 章 [[…]]）`). `scripts/check-premises.mjs` of `zt-model-restructure` finds these.
- **The derivation rests on the setting.** A derivation that starts from a setting names it as a premise (`依赖 [[setting]] 的设定：…`), never as `的结果`.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive (`D:\zt`, GUI port 4717) only when the user asks for it. Otherwise work on a copied box.
- **Back up first.** `VACUUM INTO` the archive (read-only source; the archive is in WAL mode). Name the backup after the pass, for example `zt-before-<key>-settings.sqlite3`.
- **Locks are the owner's.** A `card_locked` error means: stop, ask the user to unlock the tree in the GUI, and wait.
- **Do not change the mathematics.** Every display formula and equation number survives. Text changes are limited to moving a sentence, a pointer to a chapter, a `依赖` line, and the notation rows.
- **Move cards, never delete a Link.** Scripts move and check; never run an LLM agent per card.

## Step 1: read

- `scripts/check-model.mjs <head>`: confirm `<head>` is the Model Card. A model parted across several Model Cards is folded first (`zt-model-fold`).
- `scripts/check-settings.mjs <head>`: every fault, each with its fix:
  - `outside`: a setting or notation card filed in a chapter, with the derivations filed under it (`leave`) that stay in the chapter;
  - `inside`: a card in the setting part that is not a setting;
  - `stated`: a chapter card that states an assumption itself;
  - `notation`: a symbol missing from the notation card;
  - `missing`: no notation card.
- Read every card the check names and the cards that rest on it.

## Step 2: design the setting part

Order the setting part's Side branches as a reader meets the model: the notation card first; then the agents and information; the technology (task allocation, aggregation); the prices or wages; the timing, with each stage setting as a Side of the timing card and each equilibrium condition under the condition it refines; the distributions; the extensions (open economy, a benchmark); the data. A setting that refines another goes under it (as its Side or its Direct successor), not as one more Side of the setting card.

Becker 2025 after the pass: `a` 符号表, `b` 引言的设定, `c` 信息与努力, `d` 任务配置 (2) → `d|1` 职业平均效率与产出 (5), `e` 工资表 (7), `f` 三阶段时序 → `f|1` 进入条件 (Side `f|1|a` 自由进入使总利润为零) → `f|2` 固定成本, with the Sides `f|a` 第二阶段 and `f|b` 第三阶段, `g` 三元正态, `h` 开放经济, `i` 基准开放经济, `j` 估计的数据.

## Step 3: move

Write the moves as one script (`scripts/zt.mjs` `move`), finding each card by its title before each move, because a move renames the cards after it:

1. File each `leave` derivation in its chapter first (`before` the head of the proof chain that uses it makes it the chain's first step).
2. Park the setting part's Side branches under the last chapter card, then bring them back with the moved settings in the designed order. Side slots take only the next free letter, so a new order needs the park.
3. Move a setting that is one step of a Direct chain with `alone: true`: the chain closes up behind it.
4. Repack each chapter the settings left: park its later Side branches and bring them back from `a`.

Then the text: move each sentence that uses a derived result into the card that uses it; point the setting to its chapter; change every `依赖 [[setting]] 的结果` to `的设定`; give a derivation that starts from a moved setting its `依赖` line; fix positional words the move made wrong (`本组`, `下一张卡`, `本阶段`); add the notation rows. Write with `zt-digest call <edits> -Allow <addresses>`, then read each card back: a batch runs every call even after one fails.

## Step 4: verify and record

- `scripts/check-settings.mjs <head>` prints `settings: all in the setting part`.
- `zt-model-restructure`'s `check-model.mjs` (`self_sufficient` true), `check-network.mjs` (`network: clean`) and `check-premises.mjs` (only pointers and roadmaps left; no setting names a later result).
- `list_broken_links` is empty; the tree has the same number of Cards.
- Rebuild the head's **结构** section, record the pass in the project docs, and tell the user which tree is still unlocked.

## Helper facts (zt-digest call)

- Input is a JSON **array** of `{tool, arguments}`; one call per line of output: `N ok <address>`, `N error …`, or `N {json}` for reads.
- With `-Allow <file>` only those Cards may change, and moves are refused. Run moves **without** `-Allow`.
- `move_card` with `before` puts the moved card in that place and makes the displaced card its Direct successor: right for a new first step of a chain, wrong for reordering Side branches.
- In the Bash tool, heredocs lose backslashes; write scripts with the Write tool.
