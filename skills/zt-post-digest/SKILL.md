---
name: zt-post-digest
description: "Run the post-digest sequence of one ZET economics book after its zt-econ-digest run, given its citation key: the model passes in their fixed order (zt-model-fold on the book; then per formal model zt-model-settings, zt-model-restructure, zt-model-import with its explanation refresh; per empirical model the same passes in their empirical form, then zt-evidence-dialogue), then zt-literature-organize on the book, and zt-model-illustrate on formal models last so diagrams lock onto final addresses (docs/formal-models-proposal.md §9), with the gate each stage must pass before the next starts. Use when the user says a digest is finished and asks what comes next, asks to run, plan, order or report the model passes, the empirical passes or the literature organize of a book, or names several of these skills together; also use to write the plan of the sequence without running it."
---

# The post-digest sequence of one book

Input: a book's **citation key** (for example `goldsmithpinkham2020bartik`), written `<key>` below. Output: the book's models folded, organized, structured, sharing their blocks, illustrated and in dialogue with the rest of the box, and the book's Literature tree in the order of its source. This skill fixes the **order** and the **gates**; each stage's own skill holds its method. Load the stage's skill before you run the stage, and follow it.

The order is the owner's (2026-11-10, with illustrate added 2026-11-11). Do not reorder it, skip a stage the table says applies, or run a stage the table says does not apply, unless the user says so.

## Why this order

- **Fold first.** The digest splits one model over several Model Cards. Every later pass works on one Model Card, so it must hold the whole model.
- **Settings before restructure.** The restructure builds the spine on the setting part and keeps it as the settings pass leaves it.
- **Import after the structure.** An import names blocks (a setting, a chapter, a result with its proof). The blocks exist only after settings and restructure. Its step 8 refreshes the Derivation explanations, because the earlier passes move and extend proof chains. Never run the digest's mode `explain` for this.
- **Evidence dialogue after every import of the book.** It compares estimates with other models and Finding Cards; the comparisons are never premises, so the premises must be settled first.
- **Organize before illustrate.** Folds move Cards between sections. Organizing the Literature tree before illustrating moves each whole model once and locks in every final address, so the summary leads describe the final tree.
- **Illustrate last (formal models only).** The diagrams lock onto the final addresses of the setting part and game after Literature organize (`docs/formal-models-proposal.md` §9).

## Before the first stage

1. **The digest is finished.** `D:\zt-digest\<key>\run-report.json` has `completeness.complete: true`, or every gap it lists is understood and written down. A run still running, or a gap that is a missing digest write, stops the sequence.
2. **The live box only on the user's request.** Otherwise copy the box and run there (own `ZT_CONFIG_DIR`, own `ZT_GUI_PORT`).
3. **Back up** before each stage that writes: `zt-digest backup -Out D:\zt\backups\zt-before-<key>-<stage>.sqlite3` (runs `VACUUM INTO` + `PRAGMA integrity_check`; the legacy `node:sqlite` one-liner in `zt-model-illustrate` remains valid).
4. **Locks are the owner's.** A `card_locked` error stops the stage. Report it and wait. Check the Lock again before each stage: the owner can relock between passes.
5. **One writer on the book.** Do not run this sequence while a digest, a mode run or another session writes the same book.
6. **List the models after the fold, not before.** The fold decides how many models the book holds. Read each surviving Model Card's level with `get_model` (`formal`, `concept` or `empirical`).

## The stages

| # | Stage | Skill | Runs on | Applies to | Gate before the next stage |
|---|---|---|---|---|---|
| 1 | Fold | `zt-model-fold` | the book (`<key>`) | every Model Card | decision table written (one row per Model Card, reason in one line); `zt-check --model <head> --stage fold` (or `zt-model-fold/scripts/check-model.mjs <head>`) clean on each surviving head; `list_broken_links` empty |
| 2 | Settings | `zt-model-settings` (strict) | each Model Card | formal: setting tree, `<head>|a|a` (`符号表与读法`), **起点**/**设定**/**直觉**/**说明**, plus `判据`/`定义`/`假设 (B)`/`基准`/`边界`; empirical: data tree and specification ladder | `zt-check --model <head> --settings` (`check-settings.mjs`) clean; `zt-check --model <head> --stage settings` (`zt-model-settings/scripts/check-model.mjs`) clean |
| 3 | Restructure | `zt-model-restructure` | each Model Card | formal: intuition spine (multi-justification `结果`, mixed `推导`/`论证` branches, `基准`/`校准`/`数值实验`, `FM1`–`FM6`, and Layer 1 faults); empirical: evidence spine | `zt-check --model <head> --stage restructure`, `--network`, `--premises`, and `zt-check --model <head> --skip-shared` clean (coexisting `.mjs` equivalents: `check-model.mjs`, `check-network.mjs`, `check-premises.mjs`; only cross-model `closure` / pending `criterion` import deferred to Stage 4) |
| 4 | Import + explanations | `zt-model-import` (steps 1–8) | each Model Card | formal: shared settings/results/chapters, `校准 → 估计` (`单卡`); empirical: tested hypotheses (`单卡`) and estimated theories (`整块`) | `imports.mjs check` exits 0; `zt-check --model <head> --stage final` (`check-model.mjs --stage final`) passes (`self_sufficient: true`, strict `formal_structured: true` on L2 formal / `evidence_bound: true` on L3 empirical); `zt-check --model <head> --skip-shared` clean; step 8 `zt-digest model-explain -Step finish -Model <head>` (`explain.mjs finish`) clean (`left: 0`, `refused: 0`, `blocked: 0`) |
| 5 | Evidence dialogue | `zt-evidence-dialogue` | each **empirical** Model Card | empirical only | `dialogue.mjs check` clean; every judged pair recorded, and every supported (`same: true`) pair written as 对照, 印证 or 反驳 in **对应** |
| 6 | Literature organize | `zt-literature-organize` | the book (`<key>`) | the whole Literature tree | `zt-check --tree <key> [--snapshot <file>]` (`zt-literature-organize/scripts/check.mjs`) clean: top line follows the source's top divisions; Term list, then Cited list, at the tail; no crosser left; `list_broken_links` empty |
| 7 | Illustrate | `zt-model-illustrate` | each **formal** Model Card | formal only (owner, 2026-11-11; placed after Literature organize per `docs/formal-models-proposal.md` §9 so diagrams lock onto final addresses) | `zt-check --model <head> --charts` clean + `verify.mjs --model <head>` clean in light and dark (renders verified, lint 0, every owned setting/assumption has figure + **直觉**, every owned 定义 has **直觉**) |

**Concept models.** A concept model takes the fold only. A concept model of four parts or fewer stays as it is (the ebrill decision, 2026-10-31). Stages 2–4 run on a larger concept model only when the user asks (Stage 5 is empirical-only, Stage 6 runs on the book, and Stage 7 is formal-only).

**Order inside a stage.** Run stages 2–4 model by model in the order of the book (stage 2 on every model, then stage 3 on every model, and so on). This way an import in stage 4 finds every block of the book already built. Within stage 4, import a model before the models that take blocks from it.

**After every stage that rewrites Cards, relink the Terms**, as each stage's skill says: `zt-digest term-link -Step relink -CitationKey <key>` (or the legacy wrapper `pwsh -NoProfile -File E:\zet\scripts\book-digest\term-relink.ps1 -CitationKey <key>`). Run it once more after stage 7, then verify with `-PlanOnly`: a second plan that still changes a Card is a bug.

**Internalized binary gates (Waves 1 & 2; `docs/post-digest-binary-internalization.md`).** The stage checks and helpers below are internalized directly in `zt-check` and `zt-digest` (all existing `.mjs` scripts coexist):
- Stage gate (`check-model.mjs`): `zt-check --model <head> --stage fold|settings|restructure|final [--format json]`
- Settings audit (`check-settings.mjs`): `zt-check --model <head> --settings [--symbols] [--format json]`
- Network audit (`check-network.mjs`): `zt-check --model <head> --network [--format json]`
- Premise scan (`check-premises.mjs`): `zt-check --model <head> --premises [--format json]`
- Draft diff (`check-drafts.mjs`): `zt-check --draft-diff <orig.json> <draft.json> [--chapters a,b,...] [--format json]`
- Literature tree check (`zt-literature-organize/scripts/check.mjs`): `zt-check --tree <key> [--snapshot <file>] [--format json]`
- Chart & intuition lint/coverage (`lint.mjs` / `intuition.mjs` / non-render `verify.mjs`): `zt-check --model <head> --charts [--format json]` and `zt-check --charts-json <file> [--format json]`
- Single-model explanation refresh (`explain.mjs`): `zt-digest model-explain -Step plan|apply|finish -Model <head> [-Work <dir>] [-Force]`

## Not in this sequence

- `zt-idea-grow` (Topic trees, hubs, the register) and `zt-term-sync` (equivalent Terms across books) run only when the user asks. They read the finished Literature trees, so they come after stage 7.
- A digest mode run (`models`, `explain`, `cards`) is no stage here. A model the digest left unbuilt goes back to the digest workflow, not into this sequence.

## Record

Write `E:\zet\docs\model-passes\<key>.md`: one section per stage, with the date, the backup, the decisions taken (the fold table, the settings moved, the imports written, the pairs judged, the organize moves), the gate results and the cost of every DeepSeek job. Commit it with the stage, by pathspec.

## Report the plan without running it

When the user asks only for the sequence of a book, run nothing that writes. Read the run report and run `zt-model-fold/scripts/model-survey.mjs <key>`, which is read-only. Then report, for each stage: its skill, the Model Cards it would touch (the survey's candidate groups for stage 1; the levels and eligibility for stages 2–7), its gate, and what is still open (for example a source section with own estimates but no empirical model).
