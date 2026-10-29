---
name: zt-model-fold
description: "Decide how many models a ZET book holds and fold each model the digest split into several Model Cards back into one Model Card: survey the book's Model Cards (scripts/model-survey.mjs proposes candidate groups from rests-on Links and shared parts), decide each group by the nature of the models (the same question and primitives continued, or a model of its own), never by the size of the book, then fold every group of two or more into one Model Card, safely around zt's retype rule. Use when the user says a model is split, parted, cut into several Model Cards, wants model cards combined, merged, folded or grouped, or asks how many models a book has; give it a book's citation key. To organize each model's settings afterwards, use zt-model-settings, then zt-model-restructure, with each Model Card's address."
---

# Find a book's models and fold each into one Model Card

Input: a book's **citation key** (for example `becker2025division`). Output: the book's models, decided one by one: each model is one Model Card whose parts are every card of that model, with no broken Link, and the number of models left is a finding, written down with the reason for each. Each model then goes to `zt-model-settings` and `zt-model-restructure`, given its Model Card's address.

Tool mechanics (Workspaces, `get_model`, moves, Locks) are in the `zt` skill. A worked record (Becker 2025: 11 Model Cards, one model) is §9 of `E:\zet\docs\model-merge-proposal.md`.

## The principle: the models' nature decides, not the book's size

A book can hold one model or many. Do not assume either. A paper is not one model because it is a paper, and a long book is not many models because it is long; the number of Model Cards, their size and the number of chapters decide nothing. The digest parts models for its own reasons (one Model Card per section group, groups run in parallel and cannot see each other, the architect batches at most 8 models), so the Model Cards it left are evidence of where it cut, not of where the models are.

Decide each Model Card by what the model **is**:

- **The same model**: it keeps the other's question and its primitives (the agents, their choices, the technology or environment, the timing) and continues them. It solves one stage of the other (`本模型只求第一阶段`); it adds an extension or a variant on the same setting that changes one assumption and keeps the rest (an open economy, a matching function with increasing returns); it estimates or calibrates the model; it is the verbal version of the formal model (often in the introduction), which becomes the model's intuition chapter.
- **A model of its own**: it asks its own question or sets up its own primitives, and the book presents it as a new model. Borrowing another model's result as an input (a micro-foundation, a lemma), comparing with it, contrasting it, or reusing its notation does not make it the same model: the Link between them stays as a Link between two models. A verbal framework that several formal models fill in differently is its own model too.
- **When the nature is genuinely open** (the book never says whether a variant is a new model, and the primitives half overlap), ask the user, with the evidence for each side. Ask about nothing the evidence settles.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive (`D:\zt`, GUI port 4717) only when the user asks for it. Otherwise work on a copied box.
- **Back up first.** `VACUUM INTO` the archive (read-only source; the archive is in WAL mode, so a file copy misses changes). Name the backup after the pass, for example `zt-before-<key>-fold.sqlite3`.
- **Locks are the owner's.** A `card_locked` error means: stop, ask the user to unlock the tree in the GUI, and wait. Never work around a Lock.
- **Do not change the mathematics or the text.** The fold moves and retypes Cards; it rewrites no body except the masked Links it restores.
- **Scripts move and check.** Never run an LLM agent per card for moves or checks.

## Step 1: survey

Run `scripts/model-survey.mjs <citationKey>`. For every Model Card it prints the level, the part count, the head's question and the setting card's title; then the **candidate groups**: Model Cards joined because one rests on the other's cards or takes its parts. Under each group, `builds on` lines give that evidence, and `read` lines name the pairs linked only by kinship (`relaxes`, `corresponds`) or by comparison (`contrasts`, `illustrates`, `refers`), which never join a group by themselves.

A candidate group is a hypothesis. Links follow the digest's reading, not the book's design: a group can hold two models (one only borrows a result of the other), and two candidates can be one model (the verbal version of a formal model, which no Link joins).

## Step 2: decide by nature

For every candidate group, and for every `read` pair:

1. Read the heads and their setting cards, and the source passages where the book introduces each model (`we now extend the model`, `consider a different environment`).
2. Write down each Model Card's question and primitives side by side, and put each against the rules above: does it continue the other's primitives and question, or set up its own?
3. Split a candidate where a Model Card only borrows a result; join two candidates where one is the other's verbal version or a variant of the same setting.

Record the decision as a table, one row per Model Card: the model it belongs to, the surviving head of that model, and the reason in one line (`continues 2|a: same agents and stages, adds increasing returns in matching`; `own model: new question, borrows only the frontier of 3|a`). The number of models is the number of distinct surviving heads. Show the table to the user before folding when any row needed judgement; fold at once when the evidence settles every row.

## Step 3: fold each model

For every model with two or more Model Cards, pick its surviving head (usually its first formal Model Card). For every other Model Card X of that model, in the book's order:

1. Remove X's **结构** block: its Links to its own and to shared parts count as parts.
2. Move X's Idea-note Side successors to X's section anchor.
3. Park X's setting and its other parts as Side successors of the surviving head.
4. **Mask** every Link in X's body that targets a Model part (`scripts/mask-links.mjs mask <X>`), retype X into a part (`edit_card` with `facet`, `emphasis`, `level: ""`), then **restore** the body (`mask-links.mjs restore <X>`).
5. Move X `alone` into the head's main line, then move the parked parts back under it.

**The trap:** zt refuses to retype a Model Card while it has parts filed under it **or links any Model part in its body**. So (a) never add a rests-on line to X before the retype, (b) never retype in the same batch as moves (batched calls run even after one fails, and leave nested Model Cards), and (c) once one head becomes a part, every other head that links it must be masked too, including heads of other models that link it. Retype one Card per message and read the result.

Settings of a folded head are the model's assumptions: they go to that model's setting part, never into a chapter (the `zt-model-settings` skill's rule). A model that stays alone needs no fold. Links between two models stay as they are.

## Step 4: verify and report

- `scripts/model-survey.mjs <citationKey>` lists exactly the decided surviving heads, one per model.
- `scripts/check-model.mjs <head>` for each folded model: every former head is a part; `self_sufficient` should be true. A fault here is fixed in the restructure, by moving cards, never by deleting a Link. A model that rests on another model's card (a borrowed result) may report it; that Link is a cross-model premise and stays.
- `list_broken_links` is empty; the tree has the same number of Cards.
- Record the pass in the project docs with the decision table, tell the user how many models the book holds and why each is its own, say which tree is still unlocked, and hand each Model Card's address to `zt-model-settings`, then to `zt-model-restructure`.

## Helper facts (zt-digest call)

- Input is a JSON **array** of `{tool, arguments}`; one call per line of output: `N ok <address>`, `N error …`, or `N {json}` for reads. `scripts/zt.mjs` returns that output even when the batch exits 1 because one call failed.
- With `-Allow <file>` (one address per line) only those Cards may change, and `move_card`, `insert_card`, `delete_card` are refused. Run moves **without** `-Allow`.
- A write prints no rename map: pass `-Moves <log>` and read `{"i":N,"moved":{…}}` from the log (`scripts/zt.mjs` `move` does this).
- Side slots take only the next free letter: to reorder Side cards, park them under another card and bring them back in order.
- In the Bash tool, heredocs lose backslashes; write scripts with the Write tool.
