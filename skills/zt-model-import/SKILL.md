---
name: zt-model-import
description: "Let one ZET model use another model's building blocks (a setting, a result with its proof, a chapter, or a whole model, in the same book or another book) without copying them, given the address of its Model Card (for example hart1995firms/15|c): find what the model takes from other models (premises in another model, legacy （共用） lines, single-Card imports that leave premises behind, false shares, restated copies), write the **引入** import lines that bring each block in whole (单卡 / 整块), and check that the model reads in itself; then, as the last of the model passes, refresh its Derivation parts' explanations (scripts/explain.mjs plan | apply | finish, one DeepSeek job). Scripted with scripts/imports.mjs plan | apply | check. Use when the user wants a model to share, reuse, borrow, import or take in a setting, block, result or derivation of another model, asks why a model restates another model's setting or symbols, sees 共用 / tolerated premises / false shares / closure faults to another model's part, or wants a model's imports checked."
---

# Import building blocks into a model

Input: the **address of the Model Card** (for example `hart1995firms/15|c`), written `<head>` below. Output: a model that takes every part it uses from another model by an import line in its Model Card's `**引入**` section, keeps no copy of it, and leaves no premise behind in another model.

Tool mechanics (Workspaces, `get_model`, edits, Locks) are in the `zt` skill. How to compose Model Cards and parts is in the `zettelkasten` skill. The design is `E:\zet\docs\model-shared-blocks-proposal.md` (#205).

## What an import is

A **building block** is one part of a model with every part under its Side successors (its block: a setting with its setting tree, a result with its proof, a chapter with its results), or a whole model. It stays filed in its **home model**. Another model takes it in with one line of its Model Card's `**引入**` section:

```markdown
**引入**

- 整块 [[hart1995firms/14|a|a|5]]：总收益 R 的分段式与两类低效率
- 单卡 [[hart1995firms/14|a|a]]：沿用三时点与优先债务的设定
- 整块 [[acemoglu2010labor/2|a]]：整个劳动稀缺模型
```

- `单卡 [[X]]` takes the Card X alone. Its own premises stay in the home model (`tolerated`): a reader leaves this model to read them.
- `整块 [[X]]` takes X, every part under its Side successors, and the **closure**: every part of the home model that a Card of the block rests on. Nothing is left behind. A block stops at a chapter boundary: a Direct successor of X (the next chapter) never comes with it.
- `整块 [[<Model Card>]]` takes every part of that model.
- The text after `：` says what this model uses from the block.

What an import is **not**:
- A Link from the Model Card anywhere else (`**关联**`, 问题, 概要) to another model's part still makes it a member during the transition, but it is a **false share** (`build.false_share`): a comparison belongs in a part, an import in `**引入**`.
- A `（共用）` line of the `结构` list is the older form (a **legacy share**): `plan` turns it into an import line.
- A copy: a part of this model that restates another model's setting, symbols or result. Import the block instead, and keep in this model only what it changes.

## Empirical models: hypotheses, data and theories

An empirical model (level 3, docs/empirical-models-proposal.md §7, §9.4) is built without imports (model-compiler v2, D2): its relations to other models are written as prose Links, and this pass turns the premises among them into **引入** lines. Three kinds of block:

- **The prediction it tests.** A `假说：` part names the theory's result with `参照 [[…]]`. Import that result `单卡` in the empirical Model Card's **引入** section, and let the `假说：` part rest on it (`依赖 [[…]]` in place of `参照`). The hypothesis restates nothing: it names the sign or size the theory predicts. The import stays `单卡`: `plan` proposes no upgrade to `整块` for a result that only 假说 parts rest on, because a test takes the prediction, not its proof.
- **Shared data, a measure or an instrument.** Two studies of the same data set, index or instrument: the later imports the other's `数据：` or `测量：` part `整块`, and its own data card rests on it.
- **The theory a structural estimation estimates.** The empirical model imports the formal model whole (`整块 [[<formal Model Card>]]`); its `参数：` parts rest on the imported setting, its `反事实：` parts on its parameters and the imported results.

The other relations stay Links with their kind word first: an estimate `印证`s or `反驳`s the hypothesis it bears on; two estimates of one estimand `对照` each other; a formal model calibrated by an estimate imports that `估计：` part `单卡` into its parameter card (or cites the Cited Card with `参照` when the study is not in the box). Comparisons across books are the `zt-evidence-dialogue` pass, which runs after this one.

The explanation refresh (step 8) applies to the empirical model's `推导：估计式` parts as to any Derivation part. Its estimates have no 思路/解读: when a premise of an estimate changed, read its **量级** and **为什么** against the new premise and rewrite only what the change contradicts.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive (`D:\zt`, GUI port 4717) only when the user asks for it. Otherwise work on a copied box (`ZT_CONFIG_DIR` pointing at a config whose `archive_root` is the copy).
- **Back up first.** `VACUUM INTO` the archive (read-only source; the archive is in WAL mode). Name the backup after the pass, for example `zt-before-<key>-imports.sqlite3`.
- **Locks are the owner's.** `apply` refuses a locked tree. A `card_locked` error means: stop, ask the user to unlock the tree in the GUI, and wait.
- **Scripts write the import lines.** Never run an LLM agent per Card for this; the prose changes (a copy turned into a variant) are yours, one Card at a time.
- **Never import from this model's own range.** An `import_target` fault says so; name the part in `结构` instead.

## Steps

`scripts/imports.mjs` runs through `zt-digest call` (the `zt.mjs` helper), from this skill's folder:

1. **Check.** `node scripts/imports.mjs check <head>`. It fails (exit 1) on:
   - `import target`: an `**引入**` line that names a missing Card, a Card that is not a model's, a Card of this model, or a Model Card with `单卡`;
   - `import cycle`: an imported block that rests on this model;
   - `outside premise`: a part of this model rests on a part of another model that no import brings in;
   - `legacy share`: a `（共用）` line.

   Advice (no failure): `tolerated` premises (a reader leaves the model there), false shares, possible copies.
2. **Plan.** `node scripts/imports.mjs plan <head>` (`--json` for the data). It proposes import lines:
   - `premise`: an outside premise becomes `单卡`; several premises in one block become one `整块` line on the block's root (the others are absorbed);
   - `upgrade`: a `单卡` import whose tolerated premises its home model holds becomes `整块`;
   - `legacy`: a `（共用）` line becomes an import (`整块` when it absorbs others or its home model holds its tolerated premises);
   - `false share`: a stray Model Card Link to a part that this model's parts rest on becomes an import.

   It lists, for you to do by hand: a rests-on Link to a whole Model Card (rest on the part the step uses), a premise that is no model's part (restate it or keep it as a neighbour), a possible copy, and a false share nothing rests on (move the comparison into a part).
3. **Apply.** `node scripts/imports.mjs apply <head>` writes every proposed line (`--only <root,root>` for some). It adds lines to `**引入**` (made before `结构` when missing), rewrites an upgraded line's mode word, drops lines a `整块` line absorbs and the `（共用）` lines it replaces, then reads the model again and prints the tolerated count before and after.
4. **Repeat plan and apply** until `plan` says `nothing to import`. A first `单卡` import can leave tolerated premises; the next `plan` upgrades it to `整块`. Two rounds are normal.
5. **Copies, by hand.** For each possible copy `plan` names (`P corresponds to Q of another model`): read both. If P restates Q, import Q's block, and rewrite P as a **variant**: it rests on Q (`依赖 [[Q]]`), says how it differs (`放宽 [[Q]] 的……` for a relaxed assumption, `特例` for a special case), and states only the change; or delete P when it changes nothing (`delete_card` with `into: Q` keeps its Links). Keep the mathematics; never delete a Link to silence a check.
6. **Symbols.** An imported notation card counts as this model's. When the setting part has no notation card of its own, write one line `沿用 [[<block>]] 的全部符号` in the setting; name a symbol in this model's notation card only when this model renames it (`w 即 [[…]] 的 w_A`). Run `zt-model-settings`' `check-settings.mjs <head>`.
7. **Verify.** `check` exits 0; `get_model <head>` shows `build.self_sufficient: true`. On the GUI board each import is its own 共用 shelf (共用1, 共用2, …), the shelves stand before the setting in build order, and the lender's board carries a `⧉ n` flag on the block it lends.
8. **Refresh the explanations.** The model passes move and extend proof chains, so a Derivation part can lack its 思路 or 解读, or keep a position line (`第 k 张，共 n 张：从 … 推到 …`) that no longer matches its chain. This step replaces a run of the digest workflow in mode `explain`: it runs the digest's own explain steps on this one model, with no workflow, staging folder or run ledger. A concept model has no Derivation part and finds nothing.
   1. `node scripts/explain.mjs plan <head>`. `nothing to explain` ends the step. Otherwise it prints one JSON line with `work` and `jobs` (one job per model: `id`, `files`, `prompt`).
   2. Run each job in one DeepSeek conversation: `deepseek_run` with cwd = `work`, `prompt_files` = the job's `files`, `prompt` = its `prompt`, model `deepseek-official/deepseek-flash` (V4.1 Flash, official source only), `reasoning_effort` `high`. Approve its reads, its `zt-digest model -Step explain-check` self-check, and the writing of the one result file its prompt names; reject other writes. It ends with one JSON line.
   3. `node scripts/explain.mjs apply <head>` writes the accepted 思路/说明/解读 (it refuses a part whose 起点, steps or 结果 changed). When it lists follow-up `jobs`, send each to the conversation of the job its `continue` names (`deepseek_prompt`, `prompt_files` = `files`) and run `apply` again; the compiler allows three rounds.
   4. `node scripts/explain.mjs finish <head>` relinks Terms in the changed parts only (`zt-digest term-link -Cards`), checks that a second Term-link plan changes nothing, and plans the explanations again. It exits 0 when nothing is left and nothing was refused.
   5. Delete the conversations (`deepseek_delete`) and remove the work folder from the DeepSeek sidebar (`deepseek_remove_workspace`).

**Relink the Terms last.** A rewritten Card loses Term links, and nothing else restores them. Run `pwsh -NoProfile -File E:\zet\scripts\book-digest\term-relink.ps1 -CitationKey <key>`. It keeps every visible character, and it fails if a second plan still changes a Card.

## Where this skill sits

- After `zt-model-fold`: an import is intended sharing between two models, and the fold never joins two models because of one (only a legacy share counts as evidence of a split).
- After `zt-model-settings` and `zt-model-restructure`: both treat an imported block as premises before the setting and never audit a borrowed Card, so a model that already imports keeps its lines through them. This skill then writes the remaining import lines, settles the imported symbols (step 6) and refreshes the explanations (step 8).
- Last of the model passes: fold → settings → restructure → import. Step 8 refreshes the explanations after all of them, so a book needs no separate run of the digest in mode `explain` (that mode stays for repairing a whole book).
- The digest's model compiler writes `**引入**` lines itself (`take` with `"block": true` for 整块); this skill repairs or extends models afterwards, and migrates older models (the whole box was migrated on 2026-10-31 with `E:\zet\scripts\model-imports\migrate.mjs`).

## Report

Tell the user, per model: the import lines written (root, 单卡 / 整块, what it absorbed), tolerated before and after, the copies turned into variants (or left, with why), the false shares left as comparisons, the check result, and the explanations refreshed (parts, refused, Term links changed).
