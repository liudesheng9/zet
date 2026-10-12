# The zt-book-digest workflow

`zt-book-digest` (`.claude/workflows/zt-book-digest.js`) turns a book from the Scholar Archive into
Cards in the agent Workspace: a Literature tree for the book, Topic trees for the ideas that stand
on their own, Event Cards for the real events it reports, and a **structured model** for every
model it sets out. Run it from Claude Code ("run the zt-book-digest workflow on …"); it uses
DeepSeek Harness for the bulk reading and writing, Sonnet for the models, and Opus for the final
organization.

## What happens

```mermaid
flowchart LR
  P[Prepare<br/>Haiku] --> D1[Digest block 1<br/>Haiku drives DeepSeek ×5]
  P --> D2[Digest block 2<br/>Haiku drives DeepSeek ×5]
  P --> D3[Digest block …]
  D1 --> M1[Models for block 1<br/>Sonnet × each new Model Card]
  D2 --> M2[Models for block 2<br/>Sonnet × each new Model Card]
  D3 --> M3[Models for block …]
  M1 --> L[Link<br/>Haiku drives DeepSeek]
  M2 --> L
  M3 --> L
  L --> C[Cleanup<br/>Haiku]
  C --> O[Organize<br/>Opus]
```

| Stage | Who | What it does | What you get |
| --- | --- | --- | --- |
| **Prepare** | Haiku runs a script | Extracts the Chinese text, cuts it into section groups (about 20,000 characters each), and creates the agent Literature Card with one anchor Card per group. | `<out>\groups`, `<out>\sections`, the Literature tree `[[Key]]` → `[[Key/1]]` … |
| **Digest** | one Haiku manager per block of up to 5 groups; one DeepSeek run per group | Each DeepSeek run gets only its own sections, searches the rest of the book on disk, and writes 6–12 Cards: Idea Cards placed in Topic trees or under its anchor, Event Cards, and **one whole Model Card per model**. Blocks run in parallel. | Cards, and `owned-gNN.txt` (which Cards each group wrote) |
| **Models** | Haiku lists, then one **Sonnet** agent per Model Card | Starts for a block as soon as that block's DeepSeek runs finish, while other blocks are still digesting. Each Sonnet agent reads the Model Card, studies the book's passages behind it, corrects it, and designs its structure (below). | Model parts with charts; a report per model |
| **Link** | Haiku drives DeepSeek again | Each group's run appends reasoned Links from its own Cards to the rest of the book, including to the Model part a Card concerns. | Cross-part Links |
| **Cleanup** | Haiku | Archives the DeepSeek conversations and removes scratch files; reports DeepSeek token use. | — |
| **Organize** | Opus | Merges duplicate Topics, writes hubs, the book's structure Card and index, checks every model with `get_model`, and fixes Links. | A finished, navigable digest |

## A structured model

DeepSeek writes each model as one Card, because it reads only its own sections and must work
fast. A model rarely fits one Card, and no two models need the same parts, so in the Models stage
a Sonnet agent **designs** each model's structure:

1. It runs `scripts/book-digest/model-context.ps1`, which writes a context pack: the Model Card
   as DeepSeek wrote it, any parts it already has, the Cards that discuss it and those it links,
   its group's source files with their headings and line numbers, the group's anchor Card (the
   part's place in the book), and the book's other models.
2. It reads the source passages in full (and any other book file, Card, or Scholar Archive record
   it needs) and corrects the Model Card where it is wrong.
3. It decides the parts the model needs and names each one with its own **facet** label; marks
   the one to three parts a reader must see `core` and skippable detail `aside`; relates parts by
   Links whose sentence gives the reason and whose word right before the Link names the
   **Relation kind** (`依赖` rests on, `例示` illustrates, `对照` contrasts, `放宽` relaxes,
   `对应` corresponds, `反驳` objects); links the Cards outside the model it relates to, the same
   way; and lets parts that stand apart (a case, a variant) stay **loose**.
4. It writes the parts with charts, rewrites the Model Card as the model's entry point (问题 ·
   概要 · 结构 list · 关联 · 来源), and checks `get_model` (issues, relation kinds, neighbours)
   until it reads as designed.

For example:

```
Key/4|a        Model Card                  GHM 产权模型：……        问题 · 概要 · 结构 list · 来源
Key/4|a|a      设定                         设定：两位经理各用一项资产……
Key/4|a|a|1    假设                         假设：投资可观察但不可验证……     + table: assumption · what it buys · if relaxed
Key/4|a|a|2    议价 (core)                  议价：投资者只拿到边际回报的一半…  + plot with a slider; rests on 设定, 假设
Key/4|a|a|3    命题一 (core)                结论：所有权把激励从一方转给另一方    rests on 议价
Key/4|a|a|3|a  命题二                       结论：互补资产应同属一方            rests on 议价
Key/4|a|a|4    数值例子 (aside)             例子：R=10、r=6 时……               illustrates 命题一
Key/4|a|b      历史案例 (aside, loose)      通用汽车与费雪车身……
Key/5|b|a|2    对应 GHM (shared)            对应：剩余控制权即资产所有权……       corresponds to 议价; also linked from Key/4|a
```

- A part belongs to the Model Card it is filed under and to every Model Card that links it
  (zt 0.10, `doc/open-models-design.md`): new parts are filed under their Model Card, and a part
  two models of the book share stays where it is and is linked from the second Model Card's 结构
  list. The order of that list is the order of the divider tabs.
- Parts need not open with `本卡是模型 [[…]] 的X` nor link their Model Card; each links the parts
  it relates to in a sentence that gives the reason, with its kind word right before the Link.
  `get_model` lists the model's kinded relations, loose parts, and neighbours outside it, and
  reports only a part that belongs to no model.
- In the GUI, a Model Card or part shows divider tabs named by facet along its top edge and the
  **model board** below it: the parts laid out by what rests on what, a thread for each rests-on
  relation and a named dashed arc for every other kind, the open part lifted with its neighbours
  and their kinds and reasons, core parts legible, and the rest receding to their labels; loose
  parts lie apart, and the open Card's neighbours beyond the model are listed below. Charts are
  drawn in the card's colors, follow the light or dark theme, and open a part when clicked.
- Discussion Cards elsewhere link the part they concern, so the part's Reverse links list exactly
  the Cards that discuss it. Sonnet does not edit them; it reports the ones that should, and Opus
  acts on that in Organize.
## Running it

Arguments (`args`):

| Argument | Meaning |
| --- | --- |
| `mode` | `full` (default): everything. `link`: only the link pass (and Organize with `organize: true`) on an existing digest. `models`: only the Models stage on an existing digest's Model Cards. |
| `source`, `out`, `citationKey`, `bibtex`, `bookTitle` | the bilingual Markdown in the Scholar Archive, the staging folder, and the book's BibTeX |
| `groups` | modes `link` and `models`: the digest's group count |
| `models` | `false` skips the Models stage in a full run |
| `redoModels` | `true` restructures models that already have parts (a rerun skips them by default); use it to move models written before 0.7.0 to free facets and emphasis, or those written before 0.10.0 to Relation kinds and without boilerplate first lines |
| `perManager`, `maxChars`, `maxUnits`, `dshModel`, `dshEffort` | sizing and the DeepSeek model |

The run's result lists every group's status, every model's parts, relations by kind, neighbours, charts, and open issues, the
DeepSeek token use, and Opus's report. To structure the models of a book digested before 0.6.0 (or, with `redoModels: true`, restructure those of 0.6.0),
run it with `mode: "models"`, `citationKey`, `bookTitle`, and `out` (the Sonnet agents read the
book from the Scholar Archive when the staging folder is gone).
