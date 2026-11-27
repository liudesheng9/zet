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

- Title: `第 k 章：` plus the chapter's intuition in one line (80 characters at most). Facet: `第 k 章：<short name>`, emphasis core. The GUI shows the title without `第 k 章：` under the facet, so the title alone must read as the chapter's claim.
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
One sentence: the economic claim in plain words, matching the source's exact logical force.

**陈述**
- 出处与力度：命题 1（原文 §5.2）；力度：characterization。
- 条件与作用域：依赖 [[假设 1]]、[[均衡定义：…]]。
The exact statement, with the original formulas and equation numbers.

**为什么**
One to three sentences of intuition: why it holds, what it means.

**依据**
- 局部证明：证明见 [[first proof card]] 起；依赖 [[last proof card]]。
- （若另有外部引证或启发式）外部证明：见原文引证 [4]；启发式推导：见 [[heuristic root]]（额外假设：…）。

来源：[[citationKey]] §x
```

- Title and facet start with `结果：`; give each a specific label (`结果：命题 1 进入临界值唯一`, never `结果：结论`).
- Multi-justification honesty: local `推导`/`论证` routes, external citations (`[4]`, online supplement), heuristics with explicit extra assumptions, or `原文未证`.
- Proof branches under `结果` / `基准` may mix `推导` (algebraic chains) and `论证` (`目标` / `**依据**` / `**论证**` / `**结论**`).
- Non-theorem outputs use `基准：`, `校准：` (`**起点**` / `**参数与出处**` / `**映射说明**`), `数值实验：` (`**实验设定**` / `**数值结果**` / `**边界**`); result-level caveats use `边界：`; whole-model limits use `局限：` off the main line.

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

## Setting, Assumption, Definition & Criterion cards (setting tree)

```
一句话：what this card sets up, restricts, or defines.

**起点**
- 依赖 [[earlier setting]] 的设定：what this card uses from it.   (or: 原始设定：不依赖其他设定。)

**设定**   (or **假设** / **定义** / **判据** / **均衡定义** / **解概念**)
1. Agents, choices, timing, restrictions, or criterion conditions; one per item; formulas unchanged. A symbol the reader meets here first is defined here.

**作用域**   (required on Criterion cards and variant/regime assumptions; optional otherwise)
Which baseline, variant, strategy class, or information regime this card covers.

**直觉**
- 是什么：what this setting is in plain words.
- 为什么需要：what mechanism or comparison it enables.
- 怎样进入模型：which later setting or chapter uses it (never state a later derived result).

**说明**
Pointers to chapters or results that use this setting or establish existence/uniqueness under this criterion.

来源：[[citationKey]] §x
```

## Notation card (first Side successor of the setting, `<head>|a|a`)

- **怎样读卡片上的记号**: equation numbers, `（论文略去，此处补出）`, `推导链第 k 步，共 n 步`, what subscripts/superscripts mean.
- One table per block (or `沿用 [[<lender notation card>]] 的全部符号。` plus local override rows): `| 符号 | 含义 | 首次出现 |`, linking the Card where a reader meets each symbol first.
- **统一用语**: `| 术语 | 不用 |` (or `无特殊用语约定。` when the model adds no special term rules).

## Argument card (`论证：`, non-algebraic proof step)

```
目标：one sentence naming the claim this argument establishes (never a premise).

**依据**
- 依赖 [[设定：…]]、[[假设：…]]、[[判据：…]]：each premise once.

**论证**
Structured verbal, constructive, or graphical/phase-diagram argument.

**结论**
What this step establishes and how it closes or hands off to the next proof Card.

来源：[[citationKey]] §x
```

## Calibration (`校准：`) & Numerical Experiment (`数值实验：`) cards

- `校准：` uses `**起点**`, `**参数与出处**` (`| 参数 | 含义 | 取值 | 单位/频率 | 来源类型 | 目标或出处 |`, with per-row provenance `assigned` / `external-estimate` / `moment-matched`), and `**映射说明**`.
- `数值实验：` opens with a sentence clearly marking its numerical/calibrated status, then `**实验设定**` (linking `[[校准：…]]` or baseline setting), `**数值结果**`, and `**边界**`.

## Aside cards (example, footnote, limitation, extension)

The first sentence says what the card is and which chapter or result it belongs to; then the original content without stale relation lines.
