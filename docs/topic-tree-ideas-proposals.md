# Topic-tree Idea Cards: why they fell, and proposals

Status: 2026-10-20. The user decided that these proposals belong to a separate workflow, not to the digests: they are implemented in `zt-idea-grow` (`docs/idea-grow-design.md`, wiki `idea-grow-workflow`). The digest prompts keep their filing ladder.

## The problem

Digests now file far fewer Idea Cards in Topic trees. The total per group did not fall: each group still writes about 7–8 Idea Cards, and the target "本组约写 6–12 张卡" has not changed since the first workflow (fcc5226). What changed is where the writers file them.

| Digest | Prompt | Topic / Literature Idea Cards | Topic Cards per group | Topic share |
|---|---|---|---|---|
| hart1995firms | old | 264 / 100 | 12.6 | 73% |
| missingmarkets2026interviews | old | 72 / 15 | 12.0 | 83% |
| mascolell1995microeconomic | old | 1440 / 545 | 5.5 | 73% |
| acemoglu2000franchise | old | 44 / 19 | 5.5 | 70% |
| stiglitz2014learning (old digest) | old | 1053 / 154 | 6.2 | 87% |
| grossman2002integration (old digest) | old | 45 / 73 | 5.6 | 38% |
| pilot-90 (2026-10-18, before #95) | old | 13 / 2 | 6.5 | 87% |
| stiglitz runs 4 / 5 / 6 (2026-10-19) | new | 29/29, 183/304, 168/335 | 4.1 / 2.9 / 2.6 | 50 / 38 / 33% |
| grossman trials 1–11 (2026-10-20) | new | 4–14 / 46–63 | 0.5–1.75 | 6–22% |

Old digests come from the archive backups (`E:\zet\backups\zt.sqlite3.20261019.bak`, `D:\zt-backup-20261016-174959\zt.sqlite3`); new runs from each group's last `draft.json`. Nothing is lost after the draft: the archive holds what the writers proposed. The writers themselves choose the Literature tree.

## What Luhmann did

Niklas Luhmann kept two boxes.

- **The bibliographic box:** the source and page pointers, with no interpretation. In his second Zettelkasten it holds about 16,000 entries against about 66,000 main-box Cards (roughly one in five).
- **The main box:** his own reformulated ideas.

The method separates reading from reaction. While reading he made only bibliographic notes. He then asked how each item bore on the work in the box, and wrote what was worth keeping as a new Card in his own words. Each Card holds one idea, is readable without its source, and is filed beside the Card it continues. The fixed number lets a Card branch from any point (57/12 → 57/12a), so the box records the path of a thought, not a topic hierarchy. Links reach Cards in other places, and hub and structure Cards give the entry points. His own account of the box is "Kommunikation mit Zettelkästen" (1981); Johannes Schmidt's studies of the Bielefeld box give the figures.

In ZT terms:

- **The Literature tree is the bibliographic box.** It is the book's index, plus the notes that only make sense with the book in hand: derivation steps, the model setting, comments on the text.
- **The Topic trees are the main box.** Most of a reader's own Idea Cards belong there, filed beside the Cards they continue.

The old digests matched this (70–87% Topic share). The new ones invert it (6–22% for grossman).

## Why it happened

Ranked by effect:

1. **The filing ladder puts the Literature tree first.** In `dsh-digest-base.zh.md:50-56` the first rule that fits wins. Rule 1 keeps in the Literature tree anything "依附本书才能读懂的内容" (be96e60, 2026-10-19, #103). In an economics paper almost every claim is stated with the paper's own model, so almost every Card stops at rule 1. Line 60 still says the Topic tree is where "多数卡片放这里", which contradicts it. The ladder is deliberate; its reach is not.
2. **Rule 5 waits for a signal the writer never gets.** A new Topic is allowed only when "概念闸门已经放行新建主题". "放行" appears nowhere else, and no group prompt names a cleared concept. The concept review catalogued 0 Topics with `clears_shortlist: null` in every run, and group-finish only runs the near-title check. In an archive with no earlier agent Topics, rules 3 and 4 have no target, so rule 5 is the only way into a Topic tree, and to the writer it reads as closed. This is accidental.
3. **A writer may continue only the earlier Topic Cards in `place-beside.txt`, and only as a Side successor** (`dsh-digest-base.zh.md:59`, `calls.rs:218-235`). The file was empty in every group of trial 11 and in 64 of 67 groups of Stiglitz run 6. A group never sees the Topic Cards that earlier groups of the same run wrote. Rule 4 ("挂在那个主题最贴切的常规卡下") has no write path at all.
4. **The standalone rules (#95, 0ccd0a6, 2026-10-18) make the Literature tree the cheapest fix.**
   - Rule 14 offers "或把这张卡放进文献树".
   - The card review offers "说明这张卡属于文献树" for I7.
   - I7 flags a Topic Card that rests on its own book.
   - R20 forbids book-local numbers, so a Topic Card costs more to write than a Literature note.

   The rules are sound; the escape route they offer is the Literature tree.
5. **Minor:** the new rule that keeps Idea Cards out of Model Cards' successors files model conclusions under the anchor.

Ruled out: group size (unchanged since fcc5226), the per-group target, losses after the draft, the Topic-home transfer and join stages (the workflow never calls them), and moves from Topic to Literature (refused by policy).

## Proposals

### P1. Reverse the filing default (prompt)

Write the ladder the way Luhmann filed:

- **The book's own apparatus stays in the Literature tree:** the model setting, derivation steps, comments on the author's text, and claims that need the book's numbering. Keep this short and explicit.
- **Everything else is a Topic-tree Card:** a claim, mechanism or result a reader can state in their own words without the book's equations (for example "专用投资完成后，事后议价使投资方受损").
- **Book-bound claims that carry a general idea get two Cards:** the general claim goes in a Topic tree in the reader's words, and the book-bound note goes in the Literature tree. The Topic Card cites the book with `例示 [[…]]` or `对应 [[…]]`, so it does not rest on it (I7 stays satisfied).

Change `dsh-digest-base.zh.md:50-60` and fix the contradiction at line 60.

### P2. Make the new-Topic path concrete (prompt + data)

Replace "概念闸门已经放行" with what the writer can actually see:

- the concept review's accepted new concepts, listed in each group prompt; and
- the rule that `create_topic` is allowed whenever the snapshot holds no Topic for the concept, with group-finish's near-title check as the guard (it already refuses a duplicate Topic name).

Optionally wire the concept gate (#100) into group-finish so a refusal names the existing Topic to file under.

### P3. Let each group continue the run's own Topic Cards (data)

At group start, fill `place-beside.txt` with the agent Topic Cards that exist now: earlier groups of this run and earlier books. Allow a Direct successor when the Card is the next step of that Card's argument, and a Side successor for a branch, mirroring Luhmann's 57/12 → 57/12a. Today only Side successors of listed earlier Cards are allowed, and the list is empty. Rule 4 gets the same path: a regular Card of an existing Topic is a valid parent.

### P4. Make I7's fix write the assumption, not move the Card (prompt)

In rule 14 and the card review's I7 text, order the fixes:

1. write the needed assumption into the Card itself;
2. split it as in P1 (a general Topic Card plus a Literature note);
3. move it to the Literature tree, only when the claim truly needs the book.

### P5. A soft target and a measure (report)

The run report shows the Topic share per group and for the book. The writer prompt states the expectation: most Idea Cards of a group are Topic-tree Cards; the Literature tree holds the index and the book's apparatus. Below a threshold (say 40% for a book, 30% for a group), the audit gives advice, not a fault, so a regression like this one is seen at once.

### P6. Topic-tree branching, not flat fans (prompt)

The chain rule added on 2026-10-20 already files Literature-tree Cards by argument. Extend it to Topic trees: a new Topic Card that continues an earlier one is its Direct successor when it is the next step, and a Side successor when it is a branch or an objection. This is the main box's Folgezettel.

### P7. Keep the dedupe rules aimed at true restatements

Rules 2 and 15 and the D1 rule (an Idea Card that repeats a Model part) stay. With more Topic Cards, the restatement review should also pair new Topic Cards against Model parts and other books' Topic Cards. A general Topic Card and its book-bound Literature note (P1) are not duplicates: they make different claims and are linked by `例示`.

## Suggested order and checks

1. P1, P2 and P4 together. These are prompt edits plus one data field, and they remove the main cause.
2. P3 next. It needs group-start to write `place-beside.txt` from the live archive and `calls.rs` to accept Direct successors of listed Cards.
3. P5 with the first run after the change. Expected on grossman2002integration: a Topic share near the old digest's 38% or higher. Fewer than 2 Topic Cards per group would mean the change did not take.
4. P6 and P7 after one run's review of the new Topic trees.

## Open question for the user

How far should the shift go? The old Mas-Colell and Stiglitz digests reached 73–87% Topic share. A formal paper like grossman2002integration was at 38% even under the old prompt. A target of "most Idea Cards in Topic trees" fits Luhmann. A per-book threshold is a judgement on how much of a formal paper is the book's own apparatus.

## Sources

- Luhmann, N. (1981). "Kommunikation mit Zettelkästen. Ein Erfahrungsbericht". English translation by Manfred Kuehn, "Communicating with Slip Boxes", quoted in [Andy Matuschak's notes](https://notes.andymatuschak.org/Evergreen_notes_should_be_densely_linked) and [Revisiting Luhmann's 1981 Zettelkasten article](https://danallosso.substack.com/p/revisiting-luhmanns-1981-zettelkasten).
- Schmidt, J. F. K. (2016). "Niklas Luhmann's Card Index: Thinking Tool, Communication Partner, Publication Machine", in *Forgetting Machines* (Brill). Figures as summarized in [Niklas Luhmann's original Zettelkasten](https://www.ernestchiang.com/en/posts/2025/niklas-luhmann-original-zettelkasten-method/).
- Repository: `docs/slip-box-luhmann-design.md`, `docs/luhmann-comparison-proposals.md` (P1: the target that Topic Cards stand without their own book).
