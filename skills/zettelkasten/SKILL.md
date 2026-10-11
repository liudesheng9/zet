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
- A model first written as one Card has these body sections in order: 问题; 设定 (agents, assets, actions); 假设; 时序与信息; 求解或均衡; 主要结论; 直觉; 局限; 来源. Formulas in `$...$`. It may run longer than an Idea Card (up to about 600 words); it is still one model.
- Place it in the Literature tree as a Side successor of the anchor for the part that presents it; the derivation steps, results, and interpretations follow as its successors or live in Topic trees.
- **Every Card that derives, interprets, extends, applies, or criticises the model links to its Model Card**, or to the Model part it concerns, in a sentence saying which part of the model it concerns: `在 [[hart1995firms/7|a]] 的设定下，……`. The Reverse links then list the whole discussion of the model.

### Model structure: the Model Card and its parts

When a model deserves more than one Card (most formal models do), split it into **Model parts**: Model Cards with a `facet`, filed under the Model Card, each presenting one part of the model in depth. The Model Card becomes the model's index. **You design the structure**: which parts the model needs, what each is called, which rest on which, which are core. Inside a model the structure is a graph of reasoned Links, not a sequence or a tree.

- **Parts and facets.** One part per thing a reader may want to read, cite, or criticise on its own. The facet is a short label you choose (2–8 characters is best, 32 at most), in the Card's language: `设定`, `假设`, `议价`, `均衡与比较静态`, `命题一`, `证明梗概`, `数值例子`, `历史案例`. The common facets `setting`, `assumptions` (each assumption and what it buys; what changes if it is relaxed), `timing` (order of moves, who knows what when), `dynamics` (laws of motion, best responses, bargaining, feedback loops), `solution` (equilibrium or optimum and its derivation), `results` (propositions, comparative statics), `intuition`, and `limits` (where it stops applying; critics) are a checklist, not a template. Write only parts the source supports; a conceptual model may need two or three.
- **Relations.** A part links the parts it rests on, each in a sentence that gives the reason (`依赖时序 [[3/2|a|2]]：投资在议价之前，所以……`). These Links are the model's structure: let its shape follow the model (a chain for a derivation, a fan for several results of one mechanism, branches for two regimes) and link only real dependence, never just reading order.
- **Loose parts.** A part that rests on no part and that no part rests on (a historical case, an example, a variant, a critique) may stand loose; its first line says how it relates to the model.
- **Emphasis.** Mark `core` (`emphasis: "core"`) the one to three parts a reader must see to understand what the model shows, and `aside` the parts a reader may skip (proof details, background, numerical examples). The GUI spends a reader's attention by it, so do not mark everything core.
- **Placement.** The first part is a Side successor of the Model Card (`kind: side`); the next ones continue it as Direct successors, or branch as Side successors of a part when they form a group of their own. Filing order is the order of the divider tabs, so file core parts early and asides last. Filing is only where the Cards sit; the relations are the Links. ZT refuses a facet on a Card that is not filed under a Model Card.
- **The Model Card** keeps: 问题; a 3–5 sentence 概要 of the mechanism and what the model shows; optionally one chart of the core mechanism; a 结构 list with one line per part, `- 议价 [[3/2|a|2]]（核心）：……`; 来源. Its long sections move into the parts. Do not draw the parts and their arrows: the GUI's model board draws them from the Links.
- **Each part**: title = what this part of the model says, as a claim (`时序：先投资，部件类型揭示后再平分增益`); first line names its model, `本卡是模型 [[3/2]] 的时序。`; then the part in depth, with formulas and, where a picture explains better than prose, an HTML chart; then its relations.
- **Every part links its Model Card and the Model Card links every part.** `get_model` lists any missing Link as an issue (fix them all), and lists the model's `relations` (with the sentence of each) and its `loose` parts: check that each is what you meant.
- Discussion Cards outside the model link the part they concern: a critique of an assumption links the part that states it; an application of a result links that result's part.

### HTML charts

A chart illustrates what prose explains poorly: who interacts with whom (setting), a timeline (timing), a flow or feedback loop, a phase or best-response diagram, a plotted curve or comparative static (dynamics, results), an assumptions table (what each buys, what happens if relaxed).

- Write it in a ```` ```chart ```` fence; the GUI shows it as a figure in its own sandbox. Use HTML with inline SVG and plain JavaScript (no build step); a library over HTTPS works but needs a network. Keep each chart under about 200 lines.
- Use the card's palette so it fits both light and dark themes: `var(--zt-ink)`, `--zt-ink-soft`, `--zt-ink-faint`, `--zt-paper`, `--zt-line`, `--zt-model` and `--zt-model-soft` (the model's own ink), `--zt-accent`, `--zt-series-1` … `--zt-series-6` for data series, and `--zt-serif`, `--zt-sans`, `--zt-mono`. Do not hard-code colors or white backgrounds. Size SVG with a `viewBox` and a width up to about 440.
- TeX in the chart's HTML text (`$x_i$`, `$$...$$`) is typeset; SVG `<text>` is not, so label SVG with plain text.
- Let a reader walk the model from the chart: give an element `data-zt="3/2|a|2"` (or write `[[3/2|a|2]]` in HTML text) and a click opens that Card. Links inside a chart fence are not Links, so also list the parts in Markdown.
- Interactivity helps when it shows a mechanism: a slider for a parameter that moves a curve and an equilibrium.

### The source's own figures

When the paper's own figure says it best (an event study, the data's pattern, a diagram the argument rests on), show that figure instead of redrawing it: list the Work's figures (`list_work_artifacts`), look at the candidate, take it with `scholar_archive_artifact`, and paste the `![caption](artifact:…)` it returns. Show one figure per claim, in the Card whose claim it supports (often a `results` part or a Literature note), with a caption that says what to see in it and where it is in the source: `![图 3：民主化后 GDP 逐年上升，约 25 年后高出 20%（原文 Figure 3）](artifact:…)`. A figure is evidence, not decoration: the Card's prose still states the claim, and a chart of your own remains better for a mechanism the paper only describes.

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
- **Hub Card.** When a subject is spread over about seven or more Cards, or across trees, write a Card titled with the subject's main claim whose body lists Links to the core Cards, one line each saying what that Card contributes. Place it as a Side successor of the Card where the subject begins; a hub for a whole Topic is a Side successor of `<n>/1`, and the Topic body Links to it. Johannes Schmidt describes Luhmann's hubs as Cards carrying long lists of Links to the core Cards of a subject.
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
