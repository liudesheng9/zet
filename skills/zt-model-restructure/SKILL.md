---
name: zt-model-restructure
description: "Make a ZET model that a human can read: find a model that the digest split into several Model Cards and fold them into one Model Card, then build its inner structure as an intuition spine (a short main line of plain-language chapter cards, each chapter's results as Side branches, each result's proof chain as a Side branch of the result), with one notation card, one card format per role, and clean relations. Use when the user says a model is split, parted, messy, hard to understand, or wants model cards combined, regrouped, made more human friendly, or wants a model's inner structure or network improved; also use before you change Model Card structure by hand in any ZT tree."
---

# Restructure a ZET model for a human reader

**The one principle: lower the reader's working memory.** A reader can hold only a few things at once. Every decision below serves that: one idea per card, the claim before the detail, the intuition on the main line, the proofs out of the way until the reader asks for them. Grouping cards by *type* (all settings together, all derivations together) does **not** lower the load; it was tried on Becker 2025 and the owner rejected it. Group by *meaning*: chapter, then result, then proof.

Tool mechanics (Workspaces, `get_model`, moves, Locks) are in the `zt` skill; Card-writing rules are in `zettelkasten`. Card formats for this skill: [references/card-formats.md](references/card-formats.md). A worked record of the first use (Becker 2025, 11 Model Cards folded into one, then rebuilt as a spine) is §9 of `E:\zet\docs\model-merge-proposal.md`.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive (`D:\zt`, GUI port 4717) only when the user asks for it. Otherwise work on a copied box.
- **Back up first, every pass.** `VACUUM INTO` the archive (read-only source; the archive is in WAL mode, so a file copy misses changes). Name the backup after the pass.
- **Locks are the owner's.** A `card_locked` error means: stop, ask the user to unlock the tree in the GUI, and wait. Never work around a Lock. The owner may relock between passes; check again before each pass.
- **Do not change the mathematics.** Every display formula `$$…$$` and every equation number `(n)` survives every rewrite. Check it with a script, not by eye.
- **Scripts move and check; DeepSeek drafts text; you design.** Never run an LLM agent per card for moves or checks. Draft text through DeepSeek jobs under Haiku managers (the `deepseek-subagent-managed-protocol` skill), DeepSeek V4.1 Flash from the official source, effort high; delete every conversation afterwards.

## Step 1: find the parted model

Run `scripts/model-survey.mjs <citationKey>`. It lists every Model Card of the book with its level, part count, shared parts, and the rests-on Links into other Model Cards.

A set of Model Cards is **one model** when: later cards rest on earlier ones (Becker: `9|a` rested on `8|a` 49 times); cards take many parts in from each other (shared parts); a head says it solves only one stage of another (`本模型只求第一阶段`); they keep the same agents, symbols and timing. They are **separate** when the book changes the question or the primitives and presents a new model. Extensions the book derives from the same setting (an open economy, an estimation) are chapters of the one model unless the user says otherwise. Ask the user only about what the evidence leaves open (for example a verbal overview in the introduction).

Why they part: the digest writes one Model Card per section group, groups run in parallel and cannot see each other, and the architect batches at most 8 models.

## Step 2: fold the Model Cards into one

Pick the surviving head (usually the first formal model). For every other Model Card X:

1. Remove X's **结构** block: its Links to its own and to shared parts count as parts.
2. Move X's Idea-note Side successors to X's section anchor.
3. Park X's setting and its other parts as Side successors of the surviving head.
4. **Mask** every Link in X's body that targets a Model part (`scripts/mask-links.mjs mask <X>`), retype X into a part (`edit_card` with `facet`, `emphasis`, `level: ""`), then **restore** the body (`mask-links.mjs restore <X>`).
5. Move X `alone` into the head's main line, then move the parked parts back under it.

**The trap:** zt refuses to retype a Model Card while it has parts filed under it **or links any Model part in its body**. So (a) never add a rests-on line to X before the retype, (b) never retype in the same batch as moves (batched calls run even after one fails, and leave nested Model Cards), and (c) once one head becomes a part, every other head that links it must be masked too. Retype one Card per message and read the result.

## Step 3: design the intuition spine

Three levels; a reader holds one at a time:

```
Main line   设定 → 第 1 章：直觉 → 第 2 章 … → 第 k 章：主要结论
  └ Results   each chapter's results as its Side branches (结果：…)
       └ Proofs   each result's derivation chain as a Side branch of the result
```

- The **main line** is the setting, then chapter cards in the book's order. A chapter card states one mechanism in plain words. Put the book's verbal model (often in the introduction) first as 第 1 章：直觉.
- Under a chapter: its local setting (本章设定) first, then its results, then asides (footnotes, limitations, extensions).
- Under a result: the derivation chain that ends in it, as one Direct chain.
- All settings and the **notation card** hang under the setting card.
- A model of five or more chapters gets **parts** (分部): a `**分部**` section in the Model Card, one line per part, `- 第二部分 求解：[[<first chapter card>]] 起`. The GUI draws each part as a range of blocks on the model board and labels it on the Map; zt reports a part that starts off the main line as a build fault. New models get it from the architect's `divisions`.

Two build rules make this pass zt's checks:

- A chapter card rests (`依赖`, `用到`, `沿用`, `基于`, `代入`…) **only on earlier chapter cards**. It names its own results with a cue-free word (`结果：[[…]]`). A main-line card that rests on its own Side branch is a **forward** fault, because Side parts are built after their step.
- Every main-line card needs a rests-on path back to the setting, or it is **unanchored**.
- Write the pointer to the notation card without a premise cue: `符号与记号的含义见符号表 [[…]]` (a line with `用到` becomes a rests-on Link).

Write the target as a JSON spec (format in `scripts/tree-mover.mjs`) by card identity, then rebuild with `scripts/tree-mover.mjs plan` and `run`. The mover detaches every card leaf-first onto a parking chain, then places each card top-down into its slot, and follows every card through each move's rename map. It saves its state after every move and resumes after a failure. Freeze the tree first (it does this on the first run), and never change the spec mid-run.

## Step 4: make each card readable

1. **One notation card** as the first Side successor of the setting: every symbol grouped by block, each defined once, plus a legend of the marks (`(n)` equation numbers; `（论文略去，此处补出）` for a step the paper skipped; `推导链第 k 步，共 n 步`; subscripts). Delete the local symbol tables. Settle conflicting definitions against the source text (Becker: `π` was the circle constant, not a wage parameter).
2. **One format per role** ([references/card-formats.md](references/card-formats.md)): chapter, result, derivation, setting, aside. The claim first, in words; at most one formula on a chapter card; each premise once; no `本模型`.
3. **Consistent labels.** Facets carry the role: `第 k 章：…`, `结果：…`, `推导：…`, `设定：…`. Titles start the same way. The GUI shows a Link to a Model part by its facet (zt 0.42.1+), so the facet is what the reader sees in every Link.

Draft the text with DeepSeek: one job per chapter or per group, each with the format file, the notation file, its cards as JSON, the chain order, and the list of valid addresses. Check every draft with `scripts/check-drafts.mjs` (formulas, equation numbers, Links to existing Cards, required sections, no `本模型`). Restore verbatim any premise formula a draft dropped instead of rerunning the job. Write the cards with `zt-digest call <edits> -Allow <addresses>`.

## Step 5: verify and record

- `scripts/check-model.mjs <head>`: the main line, and the `closure`, `forward`, `reach_in`, `unanchored` faults; `self_sufficient` must be true.
- `list_broken_links` is empty; the tree has the same number of Cards plus the ones you created; no title vanished except the ones you renamed.
- Rebuild the head's **结构** section from the tree (two levels: the main line and one level under each main card).
- Look at it in the GUI: the **model board** (zt 0.43+) shows the model in blocks, one tray per chapter with its results and folded proofs, the Links between blocks as counted ribbons; and the Map's **Focus** mode (`x`) shows the low-memory view (the main line as slips, the selected chapter's results, every proof folded into `+N`); "Whole tree" unfolds everything.
- Record the pass in the project docs and tell the user which tree is still unlocked.

## Helper facts (zt-digest call)

- Input is a JSON **array** of `{tool, arguments}`; one call per line of output: `N ok <address>`, `N error …`, or `N {json}` for reads.
- With `-Allow <file>` (one address per line) only those Cards may change, and `move_card`, `insert_card`, `delete_card` are refused. Run moves **without** `-Allow`.
- A write prints no rename map: pass `-Moves <log>` and read `{"i":N,"moved":{…}}` from the log.
- `get_cards` in the `full` view can exceed the 8 MiB read limit; read 25 Cards at a time in the `text` view.
- In the Bash tool, heredocs lose backslashes; write scripts with the Write tool, and build `\` with `String.fromCharCode(92)`.
