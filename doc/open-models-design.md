# Open Models Design

Status: implemented in zt 0.10.0. It loosens the model structure of `model-structure-design.md` (zt 0.7.0) in three ways; everything that document locks and this one does not change still holds.

## Source Authority

- `CONTEXT.md` defines **Model Card**, **Model part**, **Model facet**, **Model relation**, **Relation kind**, **Loose part**, **Model neighbour**, and **Model board**.
- `model-structure-design.md` remains authoritative for storage, facets, emphasis, the rendering of math and charts, and the board's attention rules.

## Why

The 0.7 model was open inside (free facets, a graph of relations) but closed at its edges, and that made it feel restricted in use:

1. **Every relation was "rests on".** In the Hart–Moore model (`hart1995firms/11|b`) all 14 relations were written `依赖…`, though one part is an example of another (事后低效率 → 清算比例), one a critique (局限 → 假设), and one a correspondence with another model (对应 AB).
2. **A part belonged to one model, by where it was filed.** 对应 AB is as much about Aghion–Bolton (`11|a`) as about Hart–Moore, but could live in only one of them.
3. **Cards outside the model were second-class.** `52/2|b|a`, which relaxes the model's key assumption, appeared only in a 关联 paragraph and in a count ("discussed by 28"), never on the board.
4. **Every model had the same shape.** 设定 → 假设 → … → 局限 in one Direct-successor chain, each part opening `本卡是模型 [[…]] 的X`, because the consistency check required the Model Card and each part to link each other and the tools offered the common facets as a list.
5. **The model had a top.** The Model Card was a header and table of contents.

## Principles

From Niklas Luhmann, *Kommunikation mit Zettelkästen* (1981):

- **A Card's place says nothing about its rank.** Luhmann chose fixed filing numbers over any planned order by subject; a note sits where it first connected. Filing is history, not structure.
- **You can always branch inside.** The order grows from within; there is no outline decided in advance.
- **Links are the structure.** A note gets its quality from the web of references to and from it, and one note can belong to many contexts through its Links.
- **The index is entry points.** A few ways in, no master outline, no privileged top.
- **The box should surprise.** That needs internal complexity and unplanned combinations, which a template prevents.

The 0.7 model followed the first two for filing and then rebuilt a hierarchy on top. 0.10 removes it in three steps.

## Locked Model

### 1. A part belongs to a model by Link, not by where it is filed

- A **Model part** is a Model Card with a facet. Its models are the Model Card it is filed under (found by the head walk of 0.7), if any, then every other Model Card whose body links it, in Folgezettel order.
- The facet no longer needs a Model Card above the part: `a Model part (…) must be filed under its Model Card` is gone. A part may be filed anywhere, in any tree of its Workspace, and a Model Card in any tree may take it in by a Link.
- **Membership is declared by the Model Card, not by the part.** A part's own Link to a Model Card does not make it a member: parts link other models all the time (`多期推广见 [[hart1995firms/12|a]]`), and that must not drag them in. A Model Card's Link to a Model part does: the Model Card is the model's register, as Luhmann's hub Cards list the Cards of a subject. A Model Card that should only mention another model's part links that model's Model Card instead.
- `models` (in `get_card`'s `model` and in `get_model`) lists every model the Card belongs to; the structure shown is the first. Each part reports `filed` (true when it is filed under this Model Card, false when only the Model Card's Link takes it in) and `also` (the other models it serves).
- **Order.** Parts are listed in the order the Model Card first names them, then the parts filed under it that it does not name, in Folgezettel order. This is the order of the divider tabs.
- **Consistency.** The only issue left is a part in no model: ``…is a Model part (…) but belongs to no model: file it under a Model Card, or link it from one``. The Model Card need not link the parts filed under it, and a part need not link its Model Card (its Reverse links show the Model Cards that take it in).
- A Model Card with parts (filed or linked) still cannot be retyped or given a facet: ``…is the Model Card of N Model part(s) (…); change or move them first``. The check reads the stored body, so drop the Link first, then retype.
- Tree summaries still count a model once, by its Model Card. The dump format is unchanged: membership is computed, never stored.

### 2. Relations have a kind, read from the Link's sentence

- Every Link of a body is read with a **Relation kind**: the last announcing word before it in its sentence, searched from the sentence start or from the previous Link in the same sentence, whichever is later. With no such word, a Link right after another in the same sentence shares that Link's kind (`依赖 [[a]] 与 [[b]]`); otherwise it is `refers`.
- Kinds and their words (`RELATION_KINDS` in `src/model.rs`):

  | Kind | Means | Words |
  | --- | --- | --- |
  | `rests-on` | a premise or derivation step | 依赖 依据 基于 建立在 取自 沿用 承接 用到 借用 · rests on, builds on, depends on, relies on, derived from, follows from, uses |
  | `illustrates` | an example, case, or numerical illustration | 例示 举例 例子 示例 演示 例证 实例 案例 · illustrat…, example of, instance of, for example, e.g. |
  | `contrasts` | a comparison or difference | 对照 对比 相比 不同于 区别 相反 · contrast…, unlike, compare(d with), versus, differs from |
  | `relaxes` | relaxes, extends, or generalizes | 放宽 放松 推广 扩展 拓展 一般化 · relax…, extend…, generaliz… |
  | `corresponds` | a correspondence, analogy, or equivalence | 对应 类比 同构 相当于 等价 · correspond…, maps onto, analog…, equivalent, mirrors |
  | `objects` | an objection, critique, or counterexample | 反驳 批评 质疑 反例 否定 挑战 · objection, objects to, critici…, refut…, counterexample, challeng… |
  | `refers` | a plain reference | (no announcing word) |

- English words match only at the start of a word (`causes` is not `uses`); the latest-ending word wins, the longer on a tie (`counterexample` over `example`). `比较` is deliberately absent, so `比较静态` is not a contrast.
- Each relation and neighbour carries `kind`. Kinds are computed, never stored; changing the sentence changes the kind.
- Only `rests-on` relations decide the board's rows. Before 0.10 every inner Link was drawn as resting on; a Link with no announcing word is now `refers`. The digests written since 0.7 announce `依赖`, so their boards do not change.

### 3. The Model Card is an entry point, and the model opens onto the slip-box

- The Model Card's body is a convention, not a template: the question and what the model shows are what a reader needs; a 结构 list is optional (and is where Links take parts in).
- **Model neighbours.** `neighbours` lists every Link between the model's Cards (Model Card and parts) and Cards outside it, either way (`way`: `in` or `out`), with its `kind`, its sentence (`note`, in the words of the Card that writes it), and the outside Card's address, title, type, and facet. Tree roots (Topics and Literature Cards, e.g. 来源 Links) are left out. `discussion` still counts the outside Cards that link the model.
- The MCP server no longer sends `common_facets` with `get_model`, and its instructions and schemas no longer list the common facets; facets are described as what this part presents in this model, with no list or order. The common facets are still stored in one spelling, and the GUI's facet field still suggests them.

## The model board

What changes from `model-structure-design.md`:

- **Rows by `rests-on` only.** Every other kind is a dashed arc beside the two cards; while it matters (it touches the open part, or both ends are core on the Model Card) its kind is written at the arc.
- **Reasons** list the open part's relations by kind: `↑ rests on`, `↓ carries`, then `↔ illustrates / illustrated by`, `contrasts with`, `relaxes / relaxed by`, `corresponds to / mirrored by`, `objects to / objected to by`, `refers to / referred to by`, each with the part's facet and its sentence, typeset.
- **Beyond the model.** Below the reasons, the open Card's neighbours (on the Model Card, the Model Card's): `←` an outside Card that links it, `→` one it links, named by kind, announced kinds first and plain references after, six shown and the rest folded. Each opens its Card.
- **Also in.** A part that serves several models shows the other Model Cards above the board. A part taken in by Link and filed elsewhere has a dashed outline; a shared part's facet carries `⧉`.

## Interfaces

- **MCP.** `get_model` and `get_card`'s `model` gain `models`, per-part `filed` and `also`, `kind` on relations, and `neighbours`; they lose `common_facets`. `create_successor` takes a facet at any Location. The server instructions describe membership by Link and the Relation kinds.
- **GUI.** The board as above; the tab of a part in no model says how to place it.
- **Session and dump.** Unchanged.

## What was not done

- A part still needs a facet: in ZT a Model Card without a facet is a model's Model Card, so the facet is what marks a part. The facet is now any label, never a slot to fill.
- Relation kinds are inferred from words, not written as syntax (`[[a|relaxes]]`); an author who wants a kind writes the word.
- The board stays centred on the model, with the open Card's outside neighbours listed below it, rather than a free graph around any Card: that keeps the layout linear and deterministic (see the GUI attention rules).
