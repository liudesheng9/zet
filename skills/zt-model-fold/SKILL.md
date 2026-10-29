---
name: zt-model-fold
description: "Fold a ZET model that the digest split into several Model Cards back into one Model Card: survey a book's Model Cards, decide which belong to one model from the evidence (rests-on Links between them, shared parts, a head that solves only one stage of another, the same agents and symbols), then fold every other head into the surviving one as a part, safely around zt's retype rule. Use when the user says a model is split, parted, cut into several Model Cards, or wants model cards combined, merged or folded into one; give it a book's citation key. To rebuild the inner structure of the one model afterwards, use zt-model-restructure with the Model Card's address."
---

# Fold a parted model into one Model Card

Input: a book's **citation key** (for example `becker2025division`). Output: one Model Card whose parts are every card of the model, with no broken Link. Its inner structure is then rebuilt by the `zt-model-restructure` skill, given that Model Card's address.

Tool mechanics (Workspaces, `get_model`, moves, Locks) are in the `zt` skill. A worked record (Becker 2025: 11 Model Cards folded into one) is §9 of `E:\zet\docs\model-merge-proposal.md`.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive (`D:\zt`, GUI port 4717) only when the user asks for it. Otherwise work on a copied box.
- **Back up first.** `VACUUM INTO` the archive (read-only source; the archive is in WAL mode, so a file copy misses changes). Name the backup after the pass, for example `zt-before-<key>-fold.sqlite3`.
- **Locks are the owner's.** A `card_locked` error means: stop, ask the user to unlock the tree in the GUI, and wait. Never work around a Lock.
- **Do not change the mathematics or the text.** The fold moves and retypes Cards; it rewrites no body except the masked Links it restores.
- **Scripts move and check.** Never run an LLM agent per card for moves or checks.

## Step 1: find the parted model

Run `scripts/model-survey.mjs <citationKey>`. It lists every Model Card of the book with its level, part count, shared parts, and the rests-on Links into other Model Cards.

A set of Model Cards is **one model** when: later cards rest on earlier ones (Becker: `9|a` rested on `8|a` 49 times); cards take many parts in from each other (shared parts); a head says it solves only one stage of another (`本模型只求第一阶段`); they keep the same agents, symbols and timing. They are **separate** when the book changes the question or the primitives and presents a new model. Extensions the book derives from the same setting (an open economy, an estimation) belong to the one model unless the user says otherwise. Ask the user only about what the evidence leaves open (for example a verbal overview in the introduction).

Why they part: the digest writes one Model Card per section group, groups run in parallel and cannot see each other, and the architect batches at most 8 models.

## Step 2: fold the Model Cards into one

Pick the surviving head (usually the first formal model). For every other Model Card X, in the book's order:

1. Remove X's **结构** block: its Links to its own and to shared parts count as parts.
2. Move X's Idea-note Side successors to X's section anchor.
3. Park X's setting and its other parts as Side successors of the surviving head.
4. **Mask** every Link in X's body that targets a Model part (`scripts/mask-links.mjs mask <X>`), retype X into a part (`edit_card` with `facet`, `emphasis`, `level: ""`), then **restore** the body (`mask-links.mjs restore <X>`).
5. Move X `alone` into the head's main line, then move the parked parts back under it.

**The trap:** zt refuses to retype a Model Card while it has parts filed under it **or links any Model part in its body**. So (a) never add a rests-on line to X before the retype, (b) never retype in the same batch as moves (batched calls run even after one fails, and leave nested Model Cards), and (c) once one head becomes a part, every other head that links it must be masked too. Retype one Card per message and read the result.

Settings of a folded head are the model's assumptions: they go to the model's setting part, never into a chapter (the `zt-model-settings` skill's rule). Hand the Model Card's address to `zt-model-settings` next, then to `zt-model-restructure`.

## Step 3: verify the fold

- `scripts/model-survey.mjs <citationKey>` lists one Model Card for the model.
- `scripts/check-model.mjs <head>`: every former head is a part; `self_sufficient` should be true (a fault here is fixed in the restructure, by moving cards, never by deleting a Link).
- `list_broken_links` is empty; the tree has the same number of Cards.
- Record the pass in the project docs, tell the user which tree is still unlocked, and hand the Model Card's address to `zt-model-settings`, then to `zt-model-restructure`.

## Helper facts (zt-digest call)

- Input is a JSON **array** of `{tool, arguments}`; one call per line of output: `N ok <address>`, `N error …`, or `N {json}` for reads.
- With `-Allow <file>` (one address per line) only those Cards may change, and `move_card`, `insert_card`, `delete_card` are refused. Run moves **without** `-Allow`.
- A write prints no rename map: pass `-Moves <log>` and read `{"i":N,"moved":{…}}` from the log (`scripts/zt.mjs` `move` does this).
- Side slots take only the next free letter: to reorder Side cards, park them under another card and bring them back in order.
- In the Bash tool, heredocs lose backslashes; write scripts with the Write tool.
