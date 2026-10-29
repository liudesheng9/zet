# Card formats for a restructured model

Every format puts the claim first and asks the reader to hold one thing. Write in the Card's language (these examples are Chinese, as the digests are). Keep every `$$…$$` block and every equation number `(n)` of the original card.

## Chapter card (main line)

```
**直觉**
Two to four sentences, no formula: what mechanism this chapter shows and why it happens.

**本章结果**
- 结果：[[result card]] the result in one sentence.
- 结果：[[result card]] …

**承上启下**
One sentence: what this chapter takes from earlier chapters, and what the next chapter does with it.
依赖 [[earlier chapter card]] 的……：one sentence.

来源：[[citationKey]] §x
```

- Title: `第 k 章：` plus the chapter's intuition in one line (40 characters at most). Facet: `第 k 章：<short name>`, emphasis core. The GUI shows the title without `第 k 章：` under the facet, so the title alone must read as the chapter's claim.
- The first sentence of **直觉** opens the chapter's block in the model network: it states the mechanism in 20 characters or more (`这一章解第二阶段。` alone is too short; the GUI then takes the second sentence along).
- `依赖` names the earlier chapter this one builds on most: in the model network the chapter hangs under it.
- Only `结果：` before the Links to its own results: the list is the chapter's table of contents, not a premise. `依赖` and the other premise words point only to earlier chapter cards (the first chapter rests on the setting card). If a card truly uses a later card, move one of the cards; never drop the cue or the Link to pass the check.

## Parts section of the Model Card (分部)

For a model of five or more chapters, add to the Model Card, after **结构**:

```
**分部**

- 第一部分 构件：[[setting card]] 起
- 第二部分 求解：[[first chapter card of the part]] 起
```

Each line names a part by what it does and links the main-line card it starts at. Two to five parts. The GUI names each part over its run of tabs above the model network.

## Result card (Side branch of a chapter)

```
One sentence: the result, no formula.

**结果**
The exact statement, with the original formulas and equation numbers.

**为什么**
One to three sentences of intuition: why it holds, what it means.

**证明**
证明见下面的推导分支，从 [[first proof card]] 读起。
依赖 [[last proof card]] 的结果。

来源：[[citationKey]] §x
```

- Title and facet start with `结果：`; give each a specific label (`结果：进入临界值唯一`, never `结果：结论`).

## Derivation card (proof chain)

```
目的：one sentence, no formula: why this step is done.

**思路**
推导链第 k 步，共 n 步：… then two or three sentences on how.

**起点**
- 依赖 [[card]] 的结果：… (each premise once; quote its formula when the step starts from it)

**步骤**
1. … formulas unchanged; a line `说明：…` may explain why a step holds.

**结果**
$$…$$

**解读**
Two to four sentences: what the result means, and which card uses it.
```

- A temporary symbol (`H`, `C(ω)`) is defined in a sentence where it first appears.
- A step the paper skipped is marked `（论文略去，此处补出）`.

## Setting card

```
一句话：what this card sets.

**起点**
- 依赖 [[earlier setting]] 的…：what this card uses from it.   (or: 原始设定：不依赖其他设定。)
- 变体二：… (a variant's setting says which variant it is and what it changes)

**内容**
1. Agents, choices, timing, assumptions; one per item; formulas unchanged. A symbol the reader meets here first is defined here.

**为什么需要它**
One sentence: which chapter uses this setting.

符号与记号的含义见符号表 [[notation card]]。

来源：[[citationKey]] §x
```

## Notation card (first Side successor of the setting)

- **怎样读卡片上的记号**: equation numbers, `（论文略去，此处补出）`, `推导链第 k 步，共 n 步`, what `ω` and the subscripts mean.
- One table per block (economy, production, wages, costs and draws, extensions, estimation): `| 符号 | 含义 |`, each symbol once.
- **统一用语**: the one term to use for each concept.

## Aside cards (example, footnote, limitation, extension)

The first sentence says what the card is and which chapter or result it belongs to; then the original content without stale relation lines.
