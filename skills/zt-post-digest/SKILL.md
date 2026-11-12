---
name: zt-post-digest
description: "Run the post-digest sequence of one ZET economics book after its zt-econ-digest run, given its citation key: the model passes in their fixed order (zt-model-fold on the book; then per formal model zt-model-settings, zt-model-restructure, zt-model-import with its explanation refresh, zt-model-illustrate; per empirical model the same passes in their empirical form, then zt-evidence-dialogue), and zt-literature-organize on the book last, with the gate each stage must pass before the next starts. Use when the user says a digest is finished and asks what comes next, asks to run, plan, order or report the model passes, the empirical passes or the literature organize of a book, or names several of these skills together; also use to write the plan of the sequence without running it."
---

# The post-digest sequence of one book

Input: a book's **citation key** (for example `goldsmithpinkham2020bartik`), written `<key>` below. Output: the book's models folded, organized, structured, sharing their blocks, illustrated and in dialogue with the rest of the box, and the book's Literature tree in the order of its source. This skill fixes the **order** and the **gates**; each stage's own skill holds its method. Load the stage's skill before you run the stage, and follow it.

The order is the owner's (2026-11-10, with illustrate added 2026-11-11). Do not reorder it, skip a stage the table says applies, or run a stage the table says does not apply, unless the user says so.

## Why this order

- **Fold first.** The digest splits one model over several Model Cards. Every later pass works on one Model Card, so it must hold the whole model.
- **Settings before restructure.** The restructure builds the spine on the setting part and keeps it as the settings pass leaves it.
- **Import after the structure.** An import names blocks (a setting, a chapter, a result with its proof). The blocks exist only after settings and restructure. Its step 8 refreshes the Derivation explanations, because the earlier passes move and extend proof chains. Never run the digest's mode `explain` for this.
- **Illustrate after import.** The figures draw the setting part and the game as they finally stand.
- **Evidence dialogue after every import of the book.** It compares estimates with other models and Finding Cards; the comparisons are never premises, so the premises must be settled first.
- **Organize last.** Folds move Cards between sections. Organizing last moves each whole model once, and the summary leads describe the final tree.

## Before the first stage

1. **The digest is finished.** `D:\zt-digest\<key>\run-report.json` has `completeness.complete: true`, or every gap it lists is understood and written down. A run still running, or a gap that is a missing digest write, stops the sequence.
2. **The live box only on the user's request.** Otherwise copy the box and run there (own `ZT_CONFIG_DIR`, own `ZT_GUI_PORT`).
3. **Back up** before each stage that writes: `VACUUM INTO` `D:\zt\backups\zt-before-<key>-<stage>.sqlite3`, then `PRAGMA integrity_check` on the copy. The node one-liner is in `zt-model-illustrate`.
4. **Locks are the owner's.** A `card_locked` error stops the stage. Report it and wait. Check the Lock again before each stage: the owner can relock between passes.
5. **One writer on the book.** Do not run this sequence while a digest, a mode run or another session writes the same book.
6. **List the models after the fold, not before.** The fold decides how many models the book holds. Read each surviving Model Card's level with `get_model` (`formal`, `concept` or `empirical`).

## The stages

| # | Stage | Skill | Runs on | Applies to | Gate before the next stage |
|---|---|---|---|---|---|
| 1 | Fold | `zt-model-fold` | the book (`<key>`) | every Model Card | the decision table (one row per Model Card, reason in one line) is written; `check-model.mjs` clean on each surviving head; `list_broken_links` empty |
| 2 | Settings | `zt-model-settings` (strict) | each Model Card | formal: the setting tree, notation card, **起点**/**设定**/**直觉**/**说明**; empirical: the data tree and the specification ladder | `check-settings.mjs <head>` reports no fault |
| 3 | Restructure | `zt-model-restructure` | each Model Card | formal: the intuition spine; empirical: the evidence spine | `check-model.mjs`, `check-network.mjs`, `check-premises.mjs` clean; no forward or unanchored fault on the board |
| 4 | Import + explanations | `zt-model-import` (steps 1–8) | each Model Card | formal: shared settings, results, chapters; empirical: the hypotheses it tests and the theory it estimates | `imports.mjs check` exits 0; `get_model` shows `build.self_sufficient: true`; step 8 `explain.mjs finish` clean |
| 5 | Illustrate | `zt-model-illustrate` | each **formal** Model Card | formal only (owner, 2026-11-11); an empirical or concept model only on the user's word | `verify.mjs` clean in light and dark; lint 0; every setting card and 定义 part has a full **直觉** |
| 6 | Evidence dialogue | `zt-evidence-dialogue` | each **empirical** Model Card | empirical only | `dialogue.mjs check` clean; every judged pair written as 对照, 印证 or 反驳 in **对应** |
| 7 | Literature organize | `zt-literature-organize` | the book (`<key>`) | the whole Literature tree | top line follows the source's top divisions; Term list, then Cited list, at the tail; no crosser left; `list_broken_links` empty |

**Concept models.** A concept model takes the fold only. A concept model of four parts or fewer stays as it is (the ebrill decision, 2026-10-31). Stages 2–5 run on a larger concept model only when the user asks.

**Order inside a stage.** Run stages 2–4 model by model in the order of the book (stage 2 on every model, then stage 3 on every model, and so on). This way an import in stage 4 finds every block of the book already built. Within stage 4, import a model before the models that take blocks from it.

**After every stage that rewrites Cards, relink the Terms**, as each stage's skill says: `pwsh -NoProfile -File E:\zet\scripts\book-digest\term-relink.ps1 -CitationKey <key>`. Run it once more after stage 7, then plan it again: a second plan that still changes a Card is a bug.

## Not in this sequence

- `zt-idea-grow` (Topic trees, hubs, the register) and `zt-term-sync` (equivalent Terms across books) run only when the user asks. They read the finished Literature trees, so they come after stage 7.
- A digest mode run (`models`, `explain`, `cards`) is no stage here. A model the digest left unbuilt goes back to the digest workflow, not into this sequence.

## Record

Write `E:\zet\docs\model-passes\<key>.md`: one section per stage, with the date, the backup, the decisions taken (the fold table, the settings moved, the imports written, the pairs judged, the organize moves), the gate results and the cost of every DeepSeek job. Commit it with the stage, by pathspec.

## Report the plan without running it

When the user asks only for the sequence of a book, run nothing that writes. Read the run report and run `zt-model-fold/scripts/model-survey.mjs <key>`, which is read-only. Then report, for each stage: its skill, the Model Cards it would touch (the survey's candidate groups for stage 1; the levels for stages 2–6), its gate, and what is still open (for example a source section with own estimates but no empirical model).
