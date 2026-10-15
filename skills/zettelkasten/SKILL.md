---
name: zettelkasten
description: "Compose ZT Cards and organize their relationships the way Niklas Luhmann worked his Zettelkasten: one idea per Card in your own words, claim titles, Event Cards for real-world events and Model Cards for whole models, placement under ZET's Location rules (Direct and Side successors) beside the Card it continues, Links that state their reason, hub and structure Cards, Topic entry points, and Literature Cards. Use whenever you write, file, split, place, or link a Card, take notes on a paper or conversation into the slip-box, build an outline from Cards, or review a tree's structure."
---

# Zettelkasten: composing and connecting Cards

The slip-box is a **communication partner**, not a storage bin (Luhmann, 1981). It answers back only when each Card holds a processed thought and sits in a dense web of reasoned Links. Every rule below serves that. The tool mechanics (Workspaces, MCP calls, the confirm flow) are in the `zt` skill. The research behind these rules, with sources, is in [references/luhmann-and-practice.md](references/luhmann-and-practice.md); read it when the user asks why a rule exists or wants to change one.

## Filing a thought: the steps

1. **Converse first.** `search_cards` for the idea's key terms (and their synonyms, in both the user's language and English) across both Workspaces. `get_card` the best hits and read their neighbourhood: parent, Direct successor, Side successors, inbound Links. Done when you can name the Card(s) the new thought continues, refines, contradicts, or exemplifies, or you have checked the obvious terms and found none.
2. **Decide whether a Card is needed.** If an agent Card already says it, `edit_card` to sharpen it. If a human Card says it, write nothing new unless you add something; Link to it instead. Collected material that you have not turned into a claim is not a Card yet (see *Collector's fallacy* below).
3. **Compose** the Card (*Composing one Card*).
4. **Place** it (*Placement*).
5. **Link** it (*Links*). Done when every Link sits in a sentence that says why, and the Card has at least one Link besides its parent.
6. **Update the entry points**: the hub or structure Card for that line of thought, and the Topic body if the Card opens a new branch worth entering from the top (*Entry points*).
7. **Report** the addresses you created or changed, and any surprising connection you found on the way.

One thought can produce several Cards. Run steps 3 to 5 per Card and place them as a sequence, each the Direct successor of the one before.

## Composing one Card

A Card is one index card's worth of thought: Luhmann's slips were half a letter sheet, written on one side.

- **One idea.** The Card argues a single claim. If you need a sub-heading, a second claim, or "also", that part is another Card. Atomicity is the outcome to reach over time, so split a Card that has grown two ideas when you meet it.
- **Claim title.** The `# Title` is a complete sentence that states the idea, so the title alone can be cited in an argument: "央行的资产负债表负债端主要是本国货币", not "央行资产负债表". A Topic is the one exception: its title names the subject.
- **Own words.** Rewrite what you understood. Quote only a formulation remarkable in itself, keep the quote short, and mark it with its page: `"…" [[Key]] p. 12`.
- **Standalone.** A reader who arrives from a Link, years later, understands the Card without the source or its neighbours. Define a term in one sentence or Link to the Card that defines it.
- **Body shape.** About 50 to 250 words in one to three short paragraphs: the claim, the reasoning or evidence, and its scope or conditions. End with the source line when the idea came from a work: `来源：[[Key]] 第10页` or `Source: [[Key]] p. 10`.
- **Language.** Write in the language of the tree you are joining; the user's own Cards are in Chinese, so Cards that sit next to or answer them are in Chinese unless the user asks otherwise.
- **Generated parts.** End the body with your last sentence. ZT writes the Reverse links section itself below `<!-- zt:reverse-links -->`.
- **Card language.** Write plain, literal text (ASD-STE100, Simplified Technical English; for Chinese, the adaptation below). It applies to titles and bodies, not to formulas, Links, chart code, or quotations. English Cards follow the English STE rules (the `asd-ste100` skill).

  | # | Rule | Do | Do not |
  | --- | --- | --- | --- |
  | 1 | One statement per sentence | `E 违约。C 清算资产。` | `E 违约，于是 C 清算资产，同时……` |
  | 2 | At most 40 characters per sentence in a description, 30 in a derivation step (formulas and Links do not count) | Split at each new statement | A sentence that holds a condition, a cause, and a result |
  | 3 | No `；` (no `;` in English) | Two sentences | `……付不出去；……能部分付出。` |
  | 4 | Active voice: name the actor | `E 清算 $(1-f)$ 的资产。` | `资产被清算。` (`被` only when the source names no actor) |
  | 5 | A verb for an action | `分析`、`决定`、`比较` | `进行分析`、`作出决定`、`加以比较` |
  | 6 | One term, one meaning | The word in the model's term table, every time | `清算` / `卖资产` / `变卖资产` for one action |
  | 7 | At most two `的` in one noun phrase | `E 在时点 1 的现金` | `E 在时点 1 用来还债的现金的上限` |
  | 8 | Keep the source's modality | `可能` stays `可能` | Turn `可能` into a fact, or add `或许` to a fact |
  | 9 | No hedge stacking, no praise words | State the claim | `值得注意的是`、`在某种程度上`、`精妙`、`深刻` |
  | 10 | A list for three or more steps or conditions | A numbered list | Three conditions in one sentence |
  | 11 | A sentence names each display formula | `C 收支相抵，须` + `$$…$$` | A formula with no sentence |
  | 12 | A title states one claim in at most 40 characters | `融资条件：$L\ge K-w$ 且净现值为正` | A title with two claims |

  `scripts/book-digest/card-style-check.ps1` (in the zet repository) checks rules 2–7, 9, and 12 and prints one line per fault; it never changes text.

Template:

```markdown
# <one sentence stating the claim>

<The claim in your own words, with the reasoning or evidence.>

<Its scope or condition, and how it relates to neighbours: "This qualifies [[3/2]]: it holds only when …">

Source: [[Smith2024]] p. 10
```

## Event Cards and Model Cards

Every Card has a **Card type**. Most are Idea Cards (the default, everything above). Two types hold something other than an idea; set them with `type` on `create_successor` or `edit_card`. Only Regular Cards can have them.

**Event Card** (`type: event`): one event that happened in the real world, recorded as fact, not argued.

- Write one when a source reports a concrete, dated event that ideas lean on: an acquisition, a law, a court case, a bankruptcy, a crisis. A hypothetical example in a model is not an event.
- Title: the event in one sentence with its date: `1926 年通用汽车收购费雪车身`.
- Body: what happened; when and where; who acted; the source. Keep interpretation out of the description; end with one paragraph naming the ideas it is evidence for, with Links.
- Place it beside the Idea Card it is first evidence for (a Side successor), or under the Literature anchor of the part that reports it. Every Idea Card that uses the event links to it: `如 [[5/3|b]] 所记，……`.

**Model Card** (`type: model`): one whole model, stated once, completely, so that every discussion of it can rely on it.

- Write one for each model a source sets out (a formal model, or a conceptual model with explicit assumptions), before the Cards that discuss it.
- Title: the model's name and what it shows: `GHM 产权模型：资产所有权决定事前专用性投资激励`.
- A model first written as one Card opens with 问题 and what the model shows, then covers what this model needs, typically some of 设定 (agents, assets, actions), 假设, 时序与信息, 求解或均衡, 主要结论, 直觉, 局限, and ends with 来源. Formulas in `$...$`. It may run longer than an Idea Card (up to about 600 words); it is still one model, and its parts may later move into Model parts.
- Place it in the Literature tree as a Side successor of the anchor for the part that presents it; the derivation steps, results, and interpretations follow as its successors or live in Topic trees.
- **Set its elaboration level** (`level` on `create_successor` or `edit_card`): `concept` (level 1) when the source states the model in words or a diagram (a mechanism, a typology, a framework, or a formal model the source only cites); `formal` (level 2) when the source states it in mathematics (symbols, equations, a derivation, a proposition with a proof). The level decides how far to build it: a **concept model** stays the Model Card plus at most four parts in words (`设定`, `机制`, `结论`, `例子`), with no Derivation parts and no symbol-by-symbol derivation; a **formal model** is built self-sufficient, as below. When unsure, choose `formal`.
- **Every Card that derives, interprets, extends, applies, or criticises the model links to its Model Card**, or to the Model part it concerns, in a sentence saying which part of the model it concerns: `在 [[hart1995firms/7|a]] 的设定下，……`. The Reverse links then list the whole discussion of the model.

### Model structure: the Model Card and its parts

When a model deserves more than one Card (most formal models do), split it into **Model parts**: Model Cards with a `facet`, each presenting one part of the model in depth. The Model Card becomes the model's entry point: its question, what the model shows, and the Links that take its parts in. **You design the structure**: which parts the model needs, what each is called, how they relate, which are core. Follow Luhmann: where a Card is filed is history, not structure; the Links are the structure.

- **Membership.** A part belongs to the Model Card it is filed under, if any, **and to every Model Card that links it**. So file a part where it first connects (under the Model Card, or beside the Idea Card or the other model it grew from) and take it in with a Link from the Model Card; one part may serve several models (a correspondence between two models, a setting two variants share). A part's own Link to some Model Card does not make it a member: only the Model Card's Link does, so a Model Card that only mentions another model's part should link that model's Model Card instead.
- **Parts and facets.** One part per thing a reader may want to read, cite, or criticise on its own. The facet is a short label you choose (2–8 characters is best, 32 at most), in the Card's language, naming what this part presents in this model: `设定`, `议价`, `均衡与比较静态`, `命题一`, `证明梗概`, `数值例子`, `两种情形`, `历史案例`. There is no list to fill and no order: write the parts this model needs and the source supports; a conceptual model may need two or three, a model with two regimes two branches, a model whose point is a counterexample may lead with it.
- **Relations, with their kind.** A part links the parts it relates to, each in a sentence that gives the reason, and **the word before the Link says what kind of relation it is**: `依赖`/`基于`/`rests on`/`uses` (it rests on that part: a premise or derivation step), `例示`/`案例`/`for example` (it illustrates it), `对照`/`区别`/`unlike` (it contrasts with it), `放宽`/`推广`/`extends` (it relaxes or generalizes it), `对应`/`类比`/`maps onto` (it corresponds to it), `反驳`/`批评`/`counterexample` (it objects to it); with no such word the Link is a plain reference. Write the word right before the Link: `依赖时序 [[3/2|a|2]]：投资在议价之前，所以……`, `例示清算比例 [[3/2|a|3]]：例 1 中……`. Use the kind the relation really has, not `依赖` for everything: only `rests-on` stacks parts on the model board, and the other kinds are drawn beside them, named.
- **Shape.** Let the shape follow the model (a chain for a derivation, a fan for several results of one mechanism, branches for two regimes, a diamond when a result needs two mechanisms). Link only real relations, never just reading order: a long chain of parts that each rest on the previous one makes a tall, uninformative board.
- **Loose parts.** A part related to no other part (a historical case, a variant, a critique of the whole) may stand loose; its first sentence says how it relates to the model.
- **Emphasis.** Mark `core` (`emphasis: "core"`) the one to three parts a reader must see to understand what the model shows, and `aside` the parts a reader may skip (proof details, background, numerical examples). The GUI spends a reader's attention by it, so do not mark everything core.
- **A formal model is self-sufficient** (the rules below on build order, no step jumps, and Derivation parts apply to `formal` models; a `concept` model keeps its premises in the model too, but has no Derivation parts, and `get_model` lists any it has in `build.beyond_level`). A reader builds the whole model from nothing by reading only its own Cards, in build order, and never needs a Card outside the model for a premise, a definition, a symbol, or a step. So every rests-on Link from the model's range targets a member (a part filed under it, or a shared part its Model Card takes in), never another whole Model Card; Links of every other kind may go anywhere and make neighbours. A premise outside the model comes in: move it into the range (an agent Card of the same Literature tree that no other model holds), take it in from the Model Card's 结构 list (a part of another model), name and take in the parts a step uses (another Model Card), or restate it in a new part and keep the outside Card as a neighbour (a Topic-tree or human Card). A shared part's own premises stay in its home model: they are tolerated, not faults.
- **The filing order is the build order.** The first Side successor of the Model Card (`…|a`) is the setting: agents, timing, information, a **symbol table** (each symbol, what it is, where it first appears) and a **term table** (`术语表`: each term and the words not to use for it). Its Direct successors are the **main line**, the derivation step by step: each part rests only on parts before it, and a new symbol comes with a sentence that defines it. An example, numerical case, or variant is a Side successor of the step it illustrates. A correspondence, limit, or history is a later Side successor of the Model Card (`…|b`, `…|c`), outside the build. **No step jumps**: one step is what a reader checks by hand in about a minute (three to six lines of algebra, each with its reason); a longer or skipped step gets a **Derivation part** between the two parts (facet starting with `推导`, emphasis `aside`; sections 起点 / 步骤 / 结果; each line the book omits marked `（补：书中略去）`). A Card holds at most six steps, so a step that needs more becomes a **Derivation chain**: as many Derivation parts as its lines need, each starting from the result of the one before (`依赖上一步 [[…]]`); never drop lines to fit one Card. Any part of a formal model that outgrows a Card (about 12 sentences or 400 characters of prose) splits the same way. A derivation that another model also uses is a **building block**: file it once, and take in every part of its chain from the other Model Card's 结构 list. Put it there with `insert_card`, and reorder a chain with `move_card` `before` / `alone`. The divider tabs follow the build order, and the Model Card's 结构 list names the members in that order.
- **The Model Card** keeps what a reader needs to enter the model: 问题; a 3–5 sentence 概要 of the mechanism and what the model shows; optionally one chart of the core mechanism; a 结构 list naming the parts in reading order, one line each, `- 议价 [[3/2|a|2]]（核心）：……` (this list is also what takes parts filed elsewhere into the model); 关联, the model's relations to Cards outside it, each with its kind word (`放宽本模型“C 只能清算”的假设：[[52/2|b|a]] ……`); 来源. Do not draw the parts and their arrows: the GUI's model board draws them from the Links.
- **Each part**: title = what this part of the model says, as a claim (`时序：先投资，部件类型揭示后再平分增益`); then the part in depth, with formulas and, where a picture explains better than prose, an HTML chart; then its relations. It need not open with a `本卡是模型……` line or link its Model Card: the Model Card's Link takes it in, and its Reverse links show the model. Say what model it belongs to only when the sentence carries meaning (`本卡把 [[3/2]] 的议价结果推广到 n 方：……`).
- **Beyond the model.** Cards outside the model relate to the part they concern, with a kind word: a critique of an assumption `反驳 [[…]]` the part that states it, an extension `放宽 [[…]]` it, an application `例示 [[…]]` a result. `get_model` lists these Links as the model's `neighbours`, and the board shows them below the part.
- **Check** with `get_model`: `models` (every model the Card belongs to), each part's `filed` and `also`, the `relations` with their `kind` and sentence, the `loose` parts, the `neighbours`, `issues` (only a part that belongs to no model), and `build`: the members in build order with their `line`, and the faults `closure`, `forward`, and `unanchored`, each with its fix, until `self_sufficient` is true. Make every kind the one you meant: if a relation reads `refers`, put its word right before the Link.

### HTML charts

A chart illustrates what prose explains poorly: who interacts with whom (setting), a timeline (timing), a flow or feedback loop, a phase or best-response diagram, a plotted curve or comparative static (dynamics, results), an assumptions table (what each buys, what happens if relaxed).

- Write it in a ```` ```chart ```` fence; the GUI shows it as a figure in its own sandbox. Use HTML with inline SVG and plain JavaScript (no build step); a library over HTTPS works but needs a network. Keep each chart under about 200 lines.
- Use the card's palette so it fits both light and dark themes: `var(--zt-ink)`, `--zt-ink-soft`, `--zt-ink-faint`, `--zt-paper`, `--zt-line`, `--zt-model` and `--zt-model-soft` (the model's own ink), `--zt-accent`, `--zt-series-1` … `--zt-series-6` for data series, and `--zt-serif`, `--zt-sans`, `--zt-mono`. Do not hard-code colors or white backgrounds. Size SVG with a `viewBox` and a width up to about 440.
- TeX in the chart's HTML text (`$x_i$`, `$$...$$`) is typeset; SVG `<text>` is not, so label SVG with plain text.
- Let a reader walk the model from the chart: give an element `data-zt="3/2|a|2"` (or write `[[3/2|a|2]]` in HTML text) and a click opens that Card. Links inside a chart fence are not Links, so also list the parts in Markdown.
- Interactivity helps when it shows a mechanism: a slider for a parameter that moves a curve and an equilibrium.

### The source's own figures

When the paper's own figure says it best (an event study, the data's pattern, a diagram the argument rests on), show that figure instead of redrawing it: list the Work's figures (`list_work_artifacts`), look at the candidate, take it with `scholar_archive_artifact`, and paste the `![caption](artifact:…)` it returns. Show one figure per claim, in the Card whose claim it supports (often a `results` part or a Literature note), with a caption that says what to see in it and where it is in the source: `![图 3：民主化后 GDP 逐年上升，约 25 年后高出 20%（原文 Figure 3）](artifact:…)`. A figure is evidence, not decoration: the Card's prose still states the claim, and a chart of your own remains better for a mechanism the paper only describes. A Card that discusses a source figure shows it, rather than saying "see Figure 3" alone. To send the reader to the whole source (the PDF, or the Markdown at a figure), write a Document link `[原书 PDF 第 41 页](scholar:<file_id>#page=41)`; a Literature Card lists its Work's documents this way.

## ZET addresses

ZET keeps Luhmann's idea of an address that encodes where a thought grew from, but its rules differ from his drawer numbers (57/12 → 57/13, 57/12a, 57/12a1). Plan placement with ZET's rules:

- **Shape.** `<tree>/<segments>`. The tree is a Topic number (`3`) or a Citation key (`Smith2024`). Segments are joined by `|`: a number is a step along a line, a letter opens a branch.
- **Roots.** A Topic is `<n>/0`; a Literature Card is its bare Citation key. A root has exactly one successor, its first Card (`<n>/1`, `Smith2024/1`), and no Side successors.
- **Direct successor** (`kind: direct`, one per Card): `3/1 → 3/2`, `3/2|c|4 → 3/2|c|5` (a trailing number goes up by one), `3/2|c → 3/2|c|1` (after a letter, a new line starts at 1).
- **Side successor** (`kind: side`, any number per Card): the next free letter, `3/2 → 3/2|a, 3/2|b, …, 3/2|z, 3/2|aa`; `3/2|c|4 → 3/2|c|4|a`.
- **Reading** `1/2|c|4|b|b`: from `1/1`, one step to `1/2`; its third branch `c`; four steps along that branch; its second branch `b`; that Card's second branch `b`. In the GUI's Folgezettel map, steps run downward and each branch opens a new column.
- **ZT assigns the address.** You choose the parent Card (`at`) and the kind; `create_successor` returns the Location.
- **Topic numbers** come from one archive-wide sequence shared by both Workspaces and are never reused after a deletion, so agent and human Topics interleave. The number says nothing about the subject.

Where ZET differs from Luhmann's box, and what that changes in practice:

- **No insertion.** Luhmann filed 57/12a *between* 57/12 and 57/13, so a thought could enter a line anywhere. In ZET a Card has one Direct successor slot, nothing goes between `3/2` and `3/3`, and `3/2|a` stands beside `3/2` in its own column. So keep the numbered line as the spine of the argument and give the Direct slot only to its true next step. A thought that belongs between `3/2` and `3/3` becomes the Side successor `3/2|a`, and its body says so: `补在 [[3/2]] 与 [[3/3]] 之间：…`.
- **Addresses can shift.** Luhmann's numbers never changed. In ZET, deleting a Side successor moves *every* later sibling branch down one letter, with its subtree: deleting `3/2|a` turns `3/2|b` into `3/2|a`, `3/2|c` into `3/2|b`, and `3/2|c|1` into `3/2|b|1`. A move changes addresses too; ZT rewrites every `[[link]]` to match. So refer to Cards inside Cards only by `[[link]]`, and treat an address in a report to the user as valid as of now.
- **Cards can move.** Luhmann's slips stayed put. `move_card` relocates a Regular Card *with all its successors* to the next free Direct or Side slot of another agent Card, across agent Topic trees or within one Literature tree. A misplaced Card or branch can be repositioned: show the plan, then confirm with `move`.
- **Deletion takes the subtree.** `delete_card` removes the Card *and every successor under it*. Rewrite a Card that has successors instead of deleting it.
- **Two Workspaces.** Luhmann had one box; you can grow only agent trees. Continue a human line in an agent tree and Link back (rule 4 below).
- **One doorway per Topic.** Luhmann's top number was a section holding many sequences. `<n>/0` has the single successor `<n>/1` and no branches, so `<n>/1` is the tree's doorway: make it the founding claim or question, and branch the Topic's separate lines from `<n>/1` onward.

## Placement

Placement is about **conversation**, not category: a new Card goes directly behind the Card it talks to, and every other context it belongs to gets a Link. Luhmann: where a slip is stored matters little, as long as it can be reached from elsewhere. Pick the first rule that fits:

1. **The next step in a line, and that Card's Direct slot is free** ("therefore", "next", "this leads to"): its **Direct successor**. When the line already continues, go to the last Card of the line (follow the Direct successors to the end) if the thought comes after everything there; otherwise use rule 2.
2. **Everything else that grows from one specific Card**: objection, example, qualification, tangent, a second consequence, or a step that belongs mid-line. Make it a **Side successor** of that Card. A side branch is a line of its own: give it Direct successors as it develops (`3/2|a → 3/2|a|1 → 3/2|a|2`). Topic roots and Literature Cards take no Side successors; branch from `<n>/1` or `Key/1` onward.
3. **A new line of thought with no plausible neighbour** after step 1 of the filing steps: a new **Topic** (`create_topic`), whose first Card is `<n>/1` (`create_successor` at `<n>/0`). Keep the number of Topics small; check twice that the thought cannot grow from an existing Card.
4. **The thought continues a human Card.** Human trees take no agent successors, so place it in the agent Topic tree where it fits best, or in a new agent Topic, and open its body with the Link: `延续 [[0/1|a]]：…` / `Continues [[0/1|a]]: …`.

When a side branch has grown into an argument of its own (roughly seven or more Cards on a question distinct from its parent's), propose moving it to a new Topic: `create_topic`, then `move_card` the branch's first Card to `<new>/1`.

Literature has its own Zone:

- **Literature Card** (`create_literature`): the work itself. BibTeX metadata, with the DOI when one exists; the body is a page index of where things are and what they say, one line each, as the user writes it: `第10页 - 汉密尔顿建议……` / `p. 10 – Hamilton proposes …`. Literature Cards hold no opinions.
- **Literature tree Cards** (`<Key>/1`, `<Key>/1|a`): notes bound to that work, such as its argument in your own words, its method, or its data, each with page numbers.
- **Topic tree Cards**: ideas you would defend without the source. They cite the Literature Card with a Link. Most valuable Cards end up here.

## Links

A Link is a claim about a relationship, so it carries its reason. Write each `[[address]]` inside a sentence that names the relationship. Use these relationships:

| Relationship | Example sentence |
|---|---|
| continues / follows from | 由 [[3/2]] 可得：… · Follows from [[3/2]]: … |
| supports, evidence | [[Smith2024/2]] 的数据支持这一点。 · The data in [[Smith2024/2]] supports this. |
| contradicts, objection | 这与 [[1/1]] 冲突，因为… · This conflicts with [[1/1]], because … |
| qualifies, limits | 只在 [[3/4]] 的条件下成立。 · Holds only under the condition in [[3/4]]. |
| example of | [[0/2]] 是一个例子。 · [[0/2]] is an instance. |
| generalizes, same pattern elsewhere | 与 [[7/3]] 是同一机制在另一领域。 · Same mechanism as [[7/3]], in another field. |
| defines | 术语见 [[2/1]]。 · Term defined in [[2/1]]. |
| source | 来源：[[Key]] 第10页 · Source: [[Key]] p. 10 |

- Link to the most specific Card that carries the point, not to the Topic.
- Two to five Links is typical. A Card with no Links is lost: Luhmann found that unconnected slips come back only by chance.
- The most valuable Link joins distant trees, because it produces **surprise**: the same mechanism in money and in industrialisation. Look for one every time you file.
- Reverse links are generated, so each Link is written once, on the Card whose argument uses it.
- Links may cross Workspaces. Link to the user's Cards freely; they will see your Card in their Reverse links.

## Entry points, hubs, and structure Cards

The tree address gives each Card one home; these Cards give a line of thought its ways in. ZET has no keyword register: readers enter through `list_trees` (Topic titles and excerpts), `search_cards` (substring search over addresses, titles, and bodies), and the Topic Cards themselves.

- **Topic body (entry point).** Luhmann's keyword register pointed each keyword to one or two Cards, not to all of them, and the Links carried the reader on. Write a Topic body the same way: two or three sentences on the question the tree pursues, using the words a search for it would use, then Links to the one to three Cards to start reading from. `<n>/1` is always reachable from the root, so list it only when it is still the best start.
- **Findable titles.** Search matches substrings, so put the subject's usual term in the claim title (`货币`, `名义利率`), not only a pronoun or a paraphrase.
- **Hub Card.** When a subject is spread over about seven or more Cards, or across trees, write a Card of type `hub` titled with the subject's main claim. Its body opens with `范围：` (one sentence on what it covers), then `##` groups, each answering one question, and one list item per member: `- [[address]]：what this Card adds` (a lower hub gets `（枢纽）` after its Link). List the ways in, not every Card: at most about 25 lines, as Luhmann's collective reference cards held. Reach outside the tree in a `## 邻近` group, with the kind word before each Link. Set its `level`: `line` for one line of thought (a Side successor of the Card where the line begins), `topic` for a whole Topic (a Side successor of `<n>/1`, and the Topic body Links to it), `field` for several Topics (filed under the register in the Topic `索引`). A hub lists only lower hubs. Check it with `get_hub`; `list_hubs` shows which hubs are missing. The book digest's hub stage builds the agent Workspace's hubs; write one by hand only for a subject it does not cover.
- **Structure Card.** An outline of an argument, article, or answer: Links in reading order, each with the sentence it contributes. Build one before writing a long answer or a document from the slip-box; the text grows from the structure Card, and is written fresh rather than by pasting Cards together.
- Update the hub or structure Card each time you file a Card into its subject.

## Collector's fallacy

Collecting feels like learning; it is not (Christian Tietze). A slip-box fills with dead weight when Cards are excerpts, link lists without reasons, or summaries of everything read. Keep the cycle short: read, then file only the ideas you have processed into claims and connected, in the same session. A source that produced no idea gets a Literature Card with its page index and nothing more.

## Maintenance

Whenever you work in a tree:

- Split a Card that holds two ideas: keep one idea in the original, put the other in a new successor (Direct if its slot is free and the idea is the next step, otherwise Side), and Link them.
- Merge a duplicate by editing the weaker agent Card into a pointer to the stronger one. Delete it through the confirm flow only when it has no successors and no Card Links to it; deleting a Side successor also shifts its later siblings' letters, and ZT refuses a shift that would rewrite a Link inside a human Card.
- Reposition a misplaced Card or branch with `move_card` rather than deleting and recreating it, so its Links and Reverse links survive.
- Fix Links reported by `list_broken_links` in the agent Workspace.
- Write the Card that connects two existing Cards when you see a connection between them, instead of editing either one (human Cards are read-only anyway).
- Before a significant restructure (a move, a delete, many edits), show the user the plan and the affected addresses.
