---
name: zt-evidence-dialogue
description: "Write the conversation of one ZET empirical model (level 3, empirical) with the rest of the box: its estimates against the estimates of other empirical models and the Finding Cards of other books that state the same estimand, judged by DeepSeek and written by script as Links with their kind word first (对照, 印证, 反驳) in the estimate's **对应** section, in agent Cards only. Scripted with scripts/dialogue.mjs survey | apply | check. Use after zt-model-import, as the last model pass of an empirical model, or when the user wants an empirical model's estimates compared, connected or checked against other books, studies or findings; give it the address of the empirical Model Card."
---

# The evidence dialogue of an empirical model

Input: the **address of an empirical Model Card** (level 3, for example `hansen2021demand/5|c`), written `<head>` below. Output: Links between its estimates and the Cards elsewhere in the box that estimate the same thing, each with its kind word first and one reason sentence, and a check that a second survey proposes nothing new (docs/empirical-models-proposal.md §7, §9.5).

The order of the model passes for an empirical model: `zt-model-fold` → `zt-model-settings` (the data tree and the specification ladder) → `zt-model-restructure` (the evidence spine) → `zt-model-import` (hypotheses, data, theories) → this skill → `zt-literature-organize` on the book, last of the post-digest sequence (owner, 2026-11-10). The import pass turns premises into **引入** lines; this pass writes the comparisons, which are never premises.

## What it writes, and what it never does

| Relation | Written | Kind |
| --- | --- | --- |
| an estimate of another empirical model, same estimand, agreeing within its uncertainty | `印证 [[…]]：…` in both estimates' **对应** (agent Cards) | illustrates |
| the same, disagreeing | `反驳 [[…]]：…` in both | objects |
| the same estimand, not directly comparable (units, samples, measures) | `对照 [[…]]：…` in both | contrasts |
| a Finding Card of another book on the same estimand | the same three words, in our estimate's **对应** only | as above |

- **Only agent Cards carry a Link.** A human Card is never edited.
- **No new number.** The reason sentence quotes the two Cards' own numbers; nothing is converted, pooled or re-estimated.
- **No premise.** A comparison is never `依赖`. A formal model calibrated by an estimate imports it with `zt-model-import`, not here.
- **Scripts write and check; DeepSeek judges.** Never run an LLM agent per Card for the writes.

## Rules before you touch anything

- **Live box only on request.** Work on the live archive only when the user asks; otherwise on a copied box.
- **Locks are the owner's.** A `card_locked` error means: stop and ask the user. Never work around a Lock.
- **Back up first** when you work on the live box (`VACUUM INTO`, as the other model passes).

## Steps

0. **Terms first.** The survey pairs Cards by their Term links, so run `term-relink.ps1` (step 5) on the book before a survey when the model was rebuilt after the book's last Term-link pass.
1. **Survey (script).** `node scripts/dialogue.mjs survey <head> [--out <dir>]` reads the model's estimates, the model's estimand Terms (the Terms its head links in the title, **问题** and **估计对象**, with the equivalent Terms `zt-term-sync` linked in other books), and, from every Literature tree (`get_tree`, never the 200-row search), every other empirical model's estimates and the other books' Finding Cards. An estimate pairs on the Terms of what it measures (its own Term links that are not estimand Terms); an estimate with none pairs on the estimand Terms, against the other side's own text and its model head. A pair already linked either way, or already judged in this box (the judged record below), is not proposed. It writes `pairs.json` and one message file per twelve pairs (`judge-1.md`, …), after removing the message files an earlier survey left in the folder, and prints one JSON line. Survey the next model after the previous one is applied: the judged record then drops the mirror pairs. No pair: the dialogue is done; say so.
2. **Judge (DeepSeek, one conversation).** Start one DeepSeek conversation (`deepseek_run`, official source, effort high) with `references/judge.zh.md` and `judge-1.md`; send each later file with `deepseek_prompt` in the same conversation, one message at a time. Each message answers one JSON line per pair (`{"pair","same","verdict","kind","reason"}`). Collect every answer line into `answers.jsonl` in the out folder, then `deepseek_delete` the conversation.
3. **Write (script).** `node scripts/dialogue.mjs apply <head> --answers <dir>\answers.jsonl [--out <dir>]` writes one line per judged pair of the same estimand into the **对应** section (made before the source line when missing), and the reverse line into the other estimate when it is an agent Card. A pair judged `same: false`, a `kind` outside 对照/印证/反驳, and a Link already present write nothing; a same-estimand pair judged `not-comparable` is written with 对照. Every answered pair, whatever its verdict, goes into the box's judged record (`~/.zt-evidence-dialogue/judged-<ZT_CONFIG_DIR>.jsonl`, `default` for the live box), so no later survey of either model pays the judge for it again. A rerun of apply writes nothing new.
4. **Check (script).** `node scripts/dialogue.mjs check <head> [--out <dir>]`: every Card of a written Link resolves, and a second survey proposes none of the judged pairs again. A model whose survey found no pair has no apply; its check reads no links. Exit 1 names what is open.
5. **Relink the Terms** of the changed Cards: `pwsh -NoProfile -File E:\zet\scripts\book-digest\term-relink.ps1 -CitationKey <key>`.

## Cost

The ebrill2001modern/5|a pilot (2026-11-09, copied box): 54 pairs in 5 messages of one conversation, 0.125 CNY (about 0.0023 CNY a pair, cache hit 97%), about 30 s of model time; the mirror survey of 4|d|a then proposed nothing. The judge's answers were right (every pair set two different regressors side by side). Most noise comes from estimates whose regressors are not Terms of the book; they pair on the estimand Term.

## Report

Tell the user how many pairs the survey found, how many DeepSeek judged the same estimand (agree, disagree, not comparable), how many Links were written and where, and the DeepSeek cost of the judging conversation. Name every pair you left open and why.
