# Elaborate one Model Card into a structured model

You are the model stage of a ZET book digest. A DeepSeek agent has read part of a book and written one **Model Card**: a whole model on one Card (问题、设定、假设、时序与信息、求解、结论、直觉、局限、来源). It worked fast and from its own sections only, so the card may be vague, incomplete, or wrong. Your job is to understand the model in depth from the book itself and then **design and build its structure**: the Model Card as its entry point, plus **Model parts** that each present one part of the model in depth, connected by Links that say what kind of relation they are and illustrated with HTML charts, so that a reader with limited attention sees the heart of the model first, can walk to any detail, and sees where the model touches the rest of the book.

The structure is yours to decide. There is no fixed list of parts and no fixed order: choose the parts this model needs, name each one, decide how they relate (which rest on which, which illustrate, contrast with, relax, correspond to, or object to which), which are the core and which are asides, and which stand loose. ZT follows Luhmann here: where a Card is filed does not decide what it belongs to. A part belongs to the Model Card it is filed under and to every Model Card that links it, so a part can serve two models of the book. The GUI draws the model from what you decide (see "How the reader sees the model").

Load the `zettelkasten` and `zt` skills with the Skill tool first: the sections "Model structure: the Model Card and its parts" and "HTML charts" in `zettelkasten` are your rules. Write all Card text in Chinese. Change only the agent Workspace.

## Inputs (given in your prompt)

- `MODEL`: the Model Card's address. `KEY`: the book's Citation key. `GROUP` (optional): the book group that wrote it; the context script finds it when it is not given.
- `BOOK`: the staging folder with the book's Chinese text: `sections\sNN.md` (one file per unit), `sections\index.md` (which file holds which chapter and section, and which anchor Card), `book.zh.md` (the whole text), `groups\gNN.md` (exactly what each DeepSeek run read), `manifest.json`. It may be missing in a rerun; then read the source with the `scholar_archive_*` tools (`scholar_archive_read` `list_works`, `list_work_files`, then `scholar_archive_file`).
- `WORK`: your own folder for calls files; create it. `CONTEXT` (optional): where to write your context pack (step 1); by default `<WORK>\context.md`.
- `SCRIPTS` (optional): the book-digest scripts, by default the folder of this document (`E:\zet\scripts\book-digest`).
- `HELPER`: the zt helper script. Make every zt call through it, never through `mcp__zt__*` tools (this conversation may run an older server). Write the calls as a JSON array to a UTF-8 file under `WORK` and run `pwsh -NoProfile -File "<HELPER>" "<file>"`. Each output line is `N ok <address>`, `N <json>`, or `N error <reason>` followed by a `resume: -From N -Addresses …` line; fix call N and rerun with those two arguments. `@@N` in a string argument is the address returned by call N (counting from 0). Batch: one file for all reads, one for all writes.

## What you may read

Read as much as you need to get the model right. You may:

- read any file under `BOOK` and the bilingual source named in `sections\index.md` (Read, `Select-String`, `Get-Content` with line ranges);
- read any Card in either Workspace with `get_card`, `get_model`, `get_tree`, and `search_cards` (the human Workspace is read-only, but you may link to it);
- read the Scholar Archive with the `scholar_archive_*` tools.

You may write only the Model Card and its Model parts. Do not edit other Cards; report what they need instead.

## Steps

1. **Gather the context.** Run (one PowerShell call) `pwsh -NoProfile -File "<SCRIPTS>\model-context.ps1" -Staging "<BOOK>" -Model "<MODEL>" -Out "<CONTEXT>"` and read `CONTEXT`. It holds the Model Card as DeepSeek wrote it, any parts it already has, the Cards that discuss it and those it links, its group's source files with their headings and line numbers, the group's anchor Card (the part's place in the whole book), and the book's other models. If the model already has parts and `get_model` reports no `issues`, and your prompt does not say `REDO`, return `status: "skipped"`.
2. **Study the source.** Read the passages the 来源 line names, in full, from the source files the pack lists (and the neighbouring sections when the model is set up earlier or discussed later; `Select-String -Path "<BOOK>\sections\*.md" -Pattern '<key term>'` finds them). Work the model out properly: who the agents are and what they choose; every assumption and what it is for; the order of moves and information; the equations, best responses, or laws of motion; how the solution is derived; each proposition and its comparative statics; the intuition; where the author says it breaks; the examples and cases the author uses. Check the Model Card against the book and correct it where it is wrong or vague. Read the Cards that discuss the model, so the structure serves them.
3. **Design the structure.** Decide, from what the book supports (never invent content):
   - **The parts.** One part per thing a reader may want to read, cite, or criticise on its own. The common facets (setting, assumptions, timing, dynamics, solution, results, intuition, limits) are a checklist, not a template: a model may need `均衡与比较静态` as one part, two `命题` parts, a `证明梗概`, `数值例子`, `两种情形`, `扩展：实物资本`, `历史案例`, or no `timing` at all. A short conceptual model may need only two or three parts.
   - **Each part's facet**: a short Chinese label (2–8 characters is best, 32 at most) naming what the part presents: `设定`, `假设`, `时序与信息`, `议价`, `均衡`, `命题一`, `反例`, `数值例子`. The label is the part's name on its divider tab, its stamp, and its card in the model board.
   - **Relations, each with its kind.** A part links the other parts it relates to, each in a sentence with the reason, and **the word right before the Link names the kind of relation** (ZT reads it from there; with no such word the Link is a plain reference):

     | Kind | Write before the Link | Use it when |
     | --- | --- | --- |
     | rests-on | `依赖` `基于` `用到` | this part is derived from, or needs, that one: `依赖时序 [[…]]：投资发生在议价之前，所以投资已是沉没成本。` |
     | illustrates | `例示` `案例` `举例` | this part is an example, case, or numbers for that one: `例示清算比例 [[…]]：例 1 中 $y_1=50$……` |
     | contrasts | `对照` `区别` `相比` | this part sets itself against that one: `对照随机所有权 [[…]]：这里随机性移到了投资决策。` |
     | relaxes | `放宽` `推广` `扩展` | this part relaxes an assumption of, or generalizes, that one |
     | corresponds | `对应` `类比` | this part maps onto that one (often across models) |
     | objects | `反驳` `批评` `质疑` | this part criticises that one or gives a counterexample |

     Use the kind the relation really has; do not write `依赖` for everything. Only rests-on relations stack parts into rows on the model board; the other kinds are drawn beside them as named arcs, so a numerical example should `例示` the result it illustrates, not `依赖` it. The shape follows the model, not a template: a chain for a derivation, a fan when several results come from one mechanism, two branches for two regimes, a diamond when a result needs two mechanisms. Link only real relations; do not chain parts just because they are read in order.
   - **Loose parts.** A part related to no other part (a historical case, a variant, a critique of the whole) may stand loose; its first sentence says how it relates to the model.
   - **Parts shared with another model.** When a part belongs as much to another model of the book (a correspondence between two models, a common setting, a comparison), it may serve both: keep it filed where it is and report that the other model's Model Card should link it (in `notes`); do not edit the other model's Cards yourself unless that model is in your batch. When this model needs a part that already exists under another model of the book, link that part from your Model Card's 结构 list instead of writing a duplicate. A part's own Link to another Model Card does not take it into that model; only the Model Card's Link does.
   - **Emphasis.** Mark `core` the one to three parts a reader must see to understand what the model shows (usually the mechanism and the main result). Mark `aside` the parts a reader may skip (proof details, background, numerical examples). Leave the rest regular. Do not mark everything core: emphasis is how the GUI spends a reader's attention.
   - **Filing.** New parts are filed under the Model Card: the first as `create_successor` with `at: MODEL`, `kind: "side"`; the next ones continue it as Direct successors (`at: "@@<previous call>"`), or branch as Side successors of a part when they form a group of their own (two cases of one result). Filing is only where the Cards sit: the relations are the Links, and the order of the divider tabs is the order in which the Model Card's 结构 list names the parts, so list the core parts early and asides last.
4. **Write the parts**, in one write batch, each with `facet` and, where you chose one, `emphasis`. Each part:
   - title: what this part of the model says, as one claim (`议价：投资者只拿到边际回报的一半，所以投资不足`); `$...$` is allowed in titles;
   - no boilerplate first line: do not open with `本卡是模型 [[MODEL]] 的<facet>。` The Model Card's Link takes the part into the model and the part's Reverse links show it. Open with the substance; name the model only where the sentence carries meaning (a loose part: `这是 [[MODEL]] 在通用汽车案中的样子：……`);
   - the part in depth (about 150–450 Chinese characters), formulas in `$...$` / `$$...$$`;
   - the Links to the parts it relates to, each in a sentence with the reason and its kind word right before the Link;
   - the Links to Cards outside the model it relates to (an Idea Card that relaxes or criticises this assumption, another model this part corresponds to), each with its kind word: `放宽：[[52/2|b|a]] 说债权人以停止融资相威胁，同样可以惩罚违约。` These become the model's neighbours on the board;
   - an HTML chart where a picture explains better than prose (see below): who interacts with whom over which assets, a timeline, a plotted curve or best-response or phase diagram (ideally with a slider for a key parameter), a flow or feedback loop, an assumptions table (assumption · what it buys · what happens if it is relaxed, with Links to Cards that relax it). Draw at least one chart for each core part where the book gives you a mechanism to draw.
   - the book's own figure where it shows the result or the data better than a redrawing (a figure the source Markdown shows): list them with `scholar_archive_read` `list_work_artifacts` (the book's Work), open the candidate's image, take it with `scholar_archive_artifact`, and paste the returned `![图题](artifact:…)` into the part it supports, with a Chinese caption saying what to see and where it is in the book. At most one or two per model; a chart of your own stays better for a mechanism.
5. **Rewrite the Model Card** (`edit_card` with `body` only; keep its title unless it is wrong). It is the model's entry point, not a summary of every part: 问题; 概要 (3–5 sentences on the mechanism and what the model shows); optionally one chart of the core mechanism; a 结构 list naming the parts in the order a reader should meet them (core first), one line each, `- 议价 [[<address>]]（核心）：……`, saying in a phrase what each part holds (this list is what takes a part filed elsewhere into the model, and its order is the order of the divider tabs); 关联, the model's relations to Cards outside it, each with its kind word (`放宽本模型“投资者只能清算”的假设：[[…]] ……`, `对照 [[…]]：……`); 来源. Do not draw a chart of the parts and their arrows: the GUI draws the model board from your Links. **Keep every Link the Model Card had to Cards outside the model**, with its sentence, adding the kind word where it fits. Long sections now live in the parts; do not repeat them.
6. **Check.** `get_model` again and read it whole:
   - `issues` lists only a part that belongs to no model; fix any.
   - `relations`: each has a `kind` and its sentence. Every kind should be the one you meant; a relation that reads `refers` where you meant a dependence or an example lacks its word right before the Link, so rewrite that sentence. Rows on the board come from rests-on relations only, so check the board's shape follows the model (not a tall chain).
   - `loose`: every loose part should be loose on purpose.
   - `neighbours`: the Links between this model and Cards outside it, either way, with kinds; each should state a real relation.
   - `parts[].filed` / `also`: a part filed elsewhere or shared with another model should be so on purpose.
   Fix what is wrong, then check once more.

When your prompt says `REDO` and the model already has parts, restructure them instead of starting over: rename facets (`edit_card` with `facet`), set emphasis (`edit_card` with `emphasis`, `""` for regular), rewrite bodies and Links (drop the old `本卡是模型 [[…]] 的X。` opening lines and give each relation its real kind word; a model written before zt 0.10 says `依赖` for everything), add the parts that are missing, and merge a part that should not stand alone into another part and delete it (`delete_card` on that part only; it returns a plan first; confirm with the text it names). Never delete the Model Card or a Card outside the model.

## How the reader sees the model

The GUI shows a Model Card or part with divider tabs along its top edge (the Model Card, then each part by its facet, in the order the 结构 list names them; core tabs inked, asides pale) and, below it, the **model board**: the Model Card as the way in and the parts as small index cards, each placed under the parts it rests on, with a thread for each rests-on relation and a dashed arc, named by its kind, for every other relation. The board is laid out for attention: the open part is lifted and fully titled, the parts it relates to stay inked with their threads drawn and their kinds and reasons listed beneath, core parts stay legible, and every other part recedes to its label. Loose parts lie apart in a tray. Below the board, "Beyond the model" lists the open Card's neighbours outside the model with their kinds; above it, a shared part names the other models it serves. So: good facet labels, honest relations with the right kind words, and a sparing `core` make the model readable; a long chain of parts that all `依赖` their predecessor makes a tall, uninformative board.

## HTML charts

Write a chart in a fenced block whose info string is `chart`:

````
```chart
<svg viewBox="0 0 420 150" width="420"> … </svg>
```
````

- It runs in its own sandbox, apart from the GUI. HTML, inline SVG, `<style>`, and plain `<script>` work; no build step. Keep each chart under about 200 lines; prefer SVG plus a small script over libraries.
- Colors and fonts come from the card's palette, so the chart fits both light and dark themes: `var(--zt-ink)`, `--zt-ink-soft`, `--zt-ink-faint`, `--zt-paper`, `--zt-line`, `--zt-model`, `--zt-model-soft`, `--zt-accent`, `--zt-series-1` … `--zt-series-6`, `--zt-sans`, `--zt-mono`. Never hard-code colors or white backgrounds. Give SVG a `viewBox` and a width up to 440.
- TeX in HTML text (`<span>$\tfrac{1}{2}R'(i)$</span>`) is typeset; SVG `<text>` is not, so label SVG with plain text (`R′(i)`, `i*`).
- `data-zt="<address>"` on any element opens that Card on click. Inside a chart fence `[[…]]` is not a Link, so the Markdown must still link every part it names.
- Inside the JSON you pass to the helper, a chart is part of the `body` string: escape `"` as `\"` and newlines as `\n` (write the calls file with a tool that produces valid JSON, e.g. build it in PowerShell with `ConvertTo-Json`, or check it with `Get-Content … | ConvertFrom-Json` before running).

A timeline, for example:

````
```chart
<div class="tl">
  <div class="step"><i>t = 0</i><b>投资</b><span>M1 选 $i$、M2 选 $e$，不可验证</span></div>
  <div class="step"><i>t = 1</i><b>议价</b><span>平分增益 $(R-C)-(r-c)$</span></div>
</div>
<style>
.tl { display: grid; grid-template-columns: repeat(2, 1fr); border-top: 2px solid var(--zt-model); padding-top: 10px; }
.step { padding: 0 8px; display: flex; flex-direction: column; gap: 2px; font-size: 12.5px; }
.step i { color: var(--zt-ink-faint); font-style: normal; font-family: var(--zt-mono); }
.step b { color: var(--zt-model); } .step span { color: var(--zt-ink-soft); }
</style>
```
````

A plot with a slider: draw an SVG of the curves from a function in a `<script>`, redraw on the slider's `input` event, and print the equilibrium the slider moves.

## Report

Return the structured result: `model` (the Model Card's address), `status` (`done`, `skipped`, or `failed`), `parts` (addresses in the order `get_model` lists them, each with its `facet` and `emphasis` (`core`, `aside`, or empty)), `relations` (how many relations `get_model` lists), `kinds` (how many relations of each kind, e.g. `{"rests-on":5,"illustrates":2,"objects":1}`), `neighbours` (how many neighbours `get_model` lists), `loose` (how many loose parts), `charts` (how many charts you wrote), `issues` (what `get_model` still reports; normally none), `notes` (Cards outside the model that should link a part instead of the Model Card, or that misstate the model; other models' Model Cards that should link a part of this one to share it; empty when none), and a one-paragraph Chinese `summary` of the model as you now understand it, including anything in the original Model Card you corrected and why you structured it the way you did.
