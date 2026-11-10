---
name: zt-literature-organize
description: "Reorganize one ZET Literature tree so it reads in the order and division of its source material, for any discipline and any form of material: a book, paper, lecture series, report, law, interview, transcript, podcast, film, collection. The top line takes the source's own top division, lower divisions nest under it, Cards filed across divisions are reconciled, whole units stay whole, and reference lists go to the tail. Use when the user wants a Literature tree reorganized or restructured, wants it to follow the source's sections or chapters, or says a tree's Cards are out of order; give it a citation key."
---

# Organize a Literature tree by its source

Input: a Literature tree's **citation key**. Output: the same Cards, in a tree a reader can walk with the source open beside it.

**Where it sits.** After a digest or an appendix extension, this skill is the last step of the post-digest sequence of a book: `zt-model-fold` → `zt-model-settings` → `zt-model-restructure` → `zt-model-import` → `zt-evidence-dialogue` (empirical models) → this skill (owner, 2026-11-10). The model passes come first, so each whole model moves once and the summary leads describe the final tree.

Judgement decides this work. Sources differ in every way, so the scripts only show the tree and move it, and no script knows what a section is. Tool mechanics are in the `zt` skill.

## The shape to reach

- **The source's own division is the skeleton.** Take the division its author made: chapters, lectures, articles of a law, acts, episodes, essays of a collection. Only a source with no division of its own gets a derived one.
- **The top line** holds one Card per division a reader navigates by: the highest level that holds its own content. A wrapper with little of its own (the Parts of a book of chapters) yields to the level below, and its short intro opens the first division inside it. A short source may have its sections as the top line, a long one its chapters.
- **Lower divisions nest.** A subdivision is a Side branch of its division's lead Card, and the next level is a Side branch of that.
- **A lead summarizes its division.** The first Card of any division with subdivisions is a summary of that whole division, at every level, so a reader gets the division in one Card and goes down only for detail. What the old lead said goes where it belongs inside.
- **Front and back matter** with content (preface, a new edition's introduction, appendices, notes) takes its own place, where the source puts it. An appendix to one chapter stays with that chapter; an appendix to the whole work is a top Card.
- **Whole units stay whole.** A Model Card with its parts, a Theory Card, a proof chain: one thing, filed where it starts, even when it runs across divisions.
- **Reference lists are the tail.** The Term list, then the Cited list, are the last top Cards. Tools find them by role, so moving them is safe.
- **Within a division, the source's order rules**, with one exception: a digest's summary Card followed by its detail Cards is a reading aid and stays.

## Steps

1. **Read the source's structure** from its contents page or headings, or by reading it; for a source without headings, or one that is not sectioned text, see `references/source-forms.md`. The result is your written list of divisions at every level. Done when you can say which level is the top line and why.
2. **Survey the tree.** `node scripts/outline.mjs <key> --out <scratch>` saves a snapshot and prints the tree with each Card's source line. Check `list_trees`: a locked tree stops here and goes to the owner. On the live box, back up first (`VACUUM INTO`, as in `zt-model-fold`).
3. **Place every note.** For each top Card and each branch, say which division it belongs to, by its source line where it has one and by reading it where it has none. List the **crossers**: branches holding Cards of two divisions, and top Cards that are two divisions at once. Source lines are evidence in whatever form they come, and a label can repeat across divisions, so read the neighbours before you trust a bare name. Done when every note has a home.
4. **Pick the head of each division**: the Card that already introduces it, else the first Card of it. For a crosser, the head Card joins the side holding most of its content (a tie goes to the later division), and its other Cards go to their own divisions.
5. **Move.** Write ops files and run `node scripts/batch.mjs <key> <ops>`, then again with `--commit` once the preview is clean (see **Moves**).
6. **Put each division in order.** Read the outline again and `reorder` wherever a Side branch stands against the source's order.
7. **Make every lead a summary** (see **Summary leads**). Then the root Card's index, if the tree has one, lists the new top line. Links follow moves by themselves, so other Cards keep their text; correct only a sentence that a move made false.
8. **Verify.** `node scripts/check.mjs <key> <scratch>/<key>.tree.json` shows the top line and reports structural faults. Then walk the outline once against your division list from step 1. Done when every division is in place and in order.

Report the top line, the crossers and how each was settled, and any call that could go the other way.

9. **Relink the Terms last.** A rewritten Card loses Term links, and nothing else restores them. Run `pwsh -NoProfile -File E:\zet\scripts\book-digest\term-relink.ps1 -CitationKey <key>`. It keeps every visible character, and it fails if a second plan still changes a Card.

## Summary leads

Every head of a division with subdivisions gets one of two treatments. A head with no subdivisions is left alone unless a move made it false.

- **rewrite**: the head already speaks for the whole division. It stays and gets a new text.
- **demote**: the head speaks for only part of the division, usually its first subdivision. A new summary Card takes its place, and the old head becomes the first Side branch, opening the subdivision it really belongs to, retitled for that subdivision. Its passages that belong elsewhere move into the Cards they belong to, or are dropped when the summary now carries them. The Side children of its subdivision stay with it, a division's preamble Cards stand before it, and the rest move up to the summary.

A summary is written from the Cards under it, and states nothing they do not. It follows the division's order, and holds:

1. A title: the source's name for the division, `：`, then the division's main claim.
2. One or two paragraphs: the question, the path through the subdivisions, the conclusion.
3. `小节索引：`, one line per subdivision with a Link to its head.
4. `在全书中的位置：`, the previous and next divisions, with Links.
5. The source line, `来源：[[<key>]] <the division's label>`.

Write them into a leads file, in today's addresses, and run `node scripts/leads.mjs <key> <leads.json> --commit` (the entry format is at the top of the script). It goes deepest first, keeps its Links current as Cards move, and turns Term names in the new text into Term uses. Run one tree at a time, since any other write to the box stales a batch's preview; the script resumes where it stopped. Done when every division with subdivisions opens on a Card that speaks for all of it.

## Moves

A batch holds at most 64 items, commits all or nothing, and its addresses name Cards as they stood at batch start, following them through earlier items.

- `move X side_of Y`: X with its Side branches becomes Y's next Side branch. X's Direct chain stays behind and closes up.
- `move X..Z after Y`: the Direct chain X through Z becomes Y's Direct line. **Name `through` (`X..Z`) for every branch whose chain goes along**, or the chain stays behind.
- `move X before Y`: X takes Y's place and **Y becomes X's Direct successor**. To put a Side branch first, use `side_of`, then `reorder`.
- `reorder P c1 c2 ...`: names every Side child of P in the new order; a child moved earlier in the batch goes by its start address.

Addresses shift between batches, so work **from the tail of the tree to the head**, one division per ops file: earlier addresses stay valid. Each batch's `<ops>.moved.json` maps start addresses to final ones. Take a fresh outline after each pass.

A batch renames at most 2,000 Cards, and a move near the head renames every Card after it on the Direct line. For a tree too large for that, **park**: organize the last division at the tail, move its lead `side_of` a Card that stays put (the first top Card), and repeat toward the head, so each batch renames one division. Then move the leads back with `after`, the last-parked first, which shifts no other parked division. Move lists of over 1,000 Cards one per batch: park the Cited list, move the Term list to the tail, then the Cited list after it.
