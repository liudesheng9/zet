# The ZET Card Box and Luhmann's Zettelkasten: Comparison

Status: analysis and proposal (2026-10-16), with the user's decisions of 2026-10-16 (see "Decisions"). No part of this document is in the code yet. `slip-box-luhmann-design.md` holds the design that follows from it. The text follows ASD-STE100 as the other design documents do.

This document compares Niklas Luhmann's slip-box practice with the whole ZET card box as it is on `main` at `383f078`. It extends `math-cards-luhmann-comparison.md`, which compares Luhmann with the math cards only. Where the two documents name the same practice, this document repeats the user's decision of 2026-10-15 and does not change it.

## Decisions

Each practice gets one recommendation. **Accept** means that ZET follows the practice, or must follow it. **Reject** means that ZET keeps its own way, for the reason given. On 2026-10-16 the user agreed with every recommendation (Q1), approved both gaps (G1, G2), and took the recommendation for Q2 to Q4.

| No. | Luhmann's practice | Recommended | Status in ZET | Decision |
| --- | --- | --- | --- | --- |
| P1 | Fixed numbers that never change | Reject | ZET has addresses that move. ZT rewrites every Link. | Do not apply (2026-10-15) |
| P2 | No order by subject | Accept | Built | Apply (2026-10-15) |
| P3 | Internal branching | Accept | Built | Accept (2026-10-16) |
| P4 | Insertion between two slips | Accept | Built. The skill text is stale (G1). | Accept. G1 approved (2026-10-16). |
| P5 | A slip never moves | Reject | ZET moves Cards with `move_card`. | Reject (2026-10-16) |
| P6 | A link to one word in a slip | Reject | Links name a whole Card. | Reject (2026-10-16) |
| P7 | Half-sheet atomicity | Accept | Built for Idea Cards. L5 is kept for math and is not built. | Apply (2026-10-15) |
| P8 | One kind of slip, free form | Reject | ZET has Card types and templates. | Reject (2026-10-16) |
| P9 | Own words, few excerpts | Reject in the agent Workspace | Agents write. The user's own words live in the human Workspace. | Do not apply (2026-10-15) |
| P10 | The writer's own style | Reject | ZET uses the Card language (STE). | Reject (2026-10-16) |
| P11 | A separate bibliographic box | Accept | Built | Accept (2026-10-16) |
| P12 | Literature notes apart from permanent notes | Accept | Built in the book digest. L1 and L2 are kept for math and are not built. | Apply (2026-10-15) |
| P13 | References solve multiple storage | Accept | Built | Accept (2026-10-16) |
| P14 | Every reference carries its reason | Accept | Built | Accept (2026-10-16) |
| P15 | Back references by hand | Reject | ZT generates Reverse links. | Reject (2026-10-16) |
| P16 | Surprise from distant Links | Accept | Partly built. No stage looks for Links between books on purpose (G2). | Apply (L6, 2026-10-15). G2 approved (2026-10-16). |
| P17 | Unconnected slips are lost | Accept | Built | Accept (2026-10-16) |
| P18 | A keyword register outside the box, kept by hand | Accept the form, reject the place | Built as the `register` Hub Card | Accept the form, reject the place (2026-10-16) |
| P19 | Collective reference cards | Accept | Built as `topic` hubs | Accept (2026-10-16) |
| P20 | Outline cards | Accept | Built as `line` hubs and structure Cards | Accept (2026-10-16) |
| P21 | No hub of hubs | Reject | ZET has the `field` hub level. | Reject (2026-10-16) |
| P22 | Conversation with the box before filing | Accept | Built | Accept (2026-10-16) |
| P23 | Writing is not copying | Accept | Built: the Table, Spreads, structure Cards | Accept (2026-10-16) |
| P24 | One writer, slow growth | Reject | Agents digest a book in one run. | Do not apply (2026-10-15) |
| P25 | Process each source before filing (collector's fallacy) | Reject for digests, accept for hand filing | The digest covers the whole book. | Reject for digests, accept for hand filing (2026-10-16) |

The user asked for the design on 2026-10-16. Nothing goes into the code until the user asks for it.

## Accepted and rejected practices

### Accepted: ZET follows these practices

- **P2. No order by subject.** A Card is filed beside the Card it talks to. Model membership comes from Links, not from the filing place. Topic numbers are flat and group nothing.
- **P3. Internal branching.** A Direct successor continues a line. A Side successor opens a branch at one Card.
- **P4. Insertion between two slips.** `insert_card` puts a new Card into a line. ZET renumbers the line and rewrites every Link (see P1 for the reason).
- **P7. Half-sheet atomicity.** An Idea Card holds one idea in about 50 to 250 words. A Model part fits one Card (at most 12 sentences and 400 characters of prose).
- **P11. A separate bibliographic box.** A Literature Card holds the BibTeX data and a page index. Its Literature tree holds the notes bound to the work.
- **P12. Literature notes apart from permanent notes.** The book digest files most Idea Cards in Topic trees. The math digest gets this layer from L1 and L2 (kept, not built).
- **P13. References solve multiple storage.** A Card has one home. Every other context links it. One Model part can serve several models.
- **P14. Every reference carries its reason.** The kind word before a Link gives its Relation kind. `zt-check` rules L1 to L3 check the reason.
- **P16. Surprise from distant Links.** The zettelkasten skill asks for one distant Link at each filing. Topic hubs reach other trees in their `邻近` group. G2 adds a bridge stage to every digest.
- **P17. Unconnected slips are lost.** The audit lists orphans. `get_hub` lists unreached Cards. `list_broken_links` lists broken Links.
- **P18. The register gives one to three entries per term.** The `register` Hub Card follows this form. The place is different (see the rejected list).
- **P19. Collective reference cards.** A `topic` hub lists at most about 25 members, each with what it adds.
- **P20. Outline cards.** A `line` hub opens one line of thought. A structure Card orders the argument of one book.
- **P22. Conversation with the box.** The zettelkasten skill starts each filing with `search_cards`. Each digest writer gets a snapshot of all Topic Cards and can search the archive.
- **P23. Writing is not copying.** The Table lays Cards from many trees side by side. A Spread can become a Card. The skill tells the writer to write a text fresh from a structure Card.
- **P25 (hand filing only).** A source that gives no idea gets a Literature Card and nothing more.

### Rejected: ZET keeps its own way

- **P1. Fixed numbers.** Luhmann's numbers never changed. ZET addresses change when a Card moves, is inserted, or a Side successor is deleted. ZT rewrites every Link, so no reference in the archive breaks. Math units keep the book's number as a stable label (L7).
- **P5. A slip never moves.** `move_card` carries a Card with its subtree to a better place. ZT rewrites the Links, so they stay valid.
- **P6. A link to one word in a slip.** Luhmann could attach a slip to one word of another slip. A ZET Link names a whole Card. The math labels (H1, C2) name one hypothesis or clause, and that is enough.
- **P8. One kind of slip, free form.** ZET has Idea, Event, Finding, Model, and Hub Cards. Each type has a template, and `zt-check` checks it. Agents write at scale, so a fixed form is necessary for the checks.
- **P9. Own words, few excerpts.** The agent Workspace serves the agents' comprehension. The user's own comprehension lives in the human Workspace. No feature writes Cards into the human Workspace.
- **P10. The writer's own style.** Card titles and bodies follow the Card language (ASD-STE100, adapted for Chinese). An agent reads the Cards, so a single meaning per sentence is more important than a personal style.
- **P15. Back references by hand.** ZT generates the Reverse links, so they cannot go stale.
- **P18 (the place and the upkeep).** Luhmann kept the register outside the box and rewrote it four times by hand. A stale pointer still survived. The ZET register is a Card in the Topic `索引`. `list_trees` shows it, and the hub compiler rebuilds it.
- **P21. No hub of hubs.** Luhmann's section numbers grouped his subjects. ZET Topic numbers group nothing, and the agent Workspace grew to more than 130 Topics in days. The `field` hub level does the job of his section numbers.
- **P24. One writer, slow growth.** Agents write a whole book in one run. Size rules replace the "squeeze point" that a single writer feels.
- **P25 (digests).** A digest covers every section of a book. Its job is a complete and checked copy of the book's ideas, models, events, and findings, not a selection by one reader.

## Source Authority

- Luhmann's practice: `skills/zettelkasten/references/luhmann-and-practice.md` and the section "What Luhmann did" in `hub-cards-design.md`. They give the evidence: Luhmann 1981 and the 1968 lecture, Schmidt 2016 and 2018, and the Bielefeld archive. They also give the later practice of Ahrens, Matuschak, zettelkasten.de, and Doto.
- The earlier comparison and its decisions: `math-cards-luhmann-comparison.md` (decisions of 2026-10-15).
- The current card box: `CONTEXT.md`, `skills/zettelkasten/SKILL.md`, `docs/wiki/`, `hub-cards-design.md`, `table-and-spreads-design.md`, `zt-book-digest-workflow.md`, and the digest prompts in `scripts/book-digest/`.
- This analysis read no new external source. The Luhmann facts come only from the two documents above.

## Short answer

ZET follows Luhmann closely in its **mechanics**. It has branching addresses and one home per Card, with Links for every other place. Its Links state their reason. Its register gives few entries per term, and its hubs reach other trees. In some places ZET is stronger than Luhmann: Links follow a moved Card, ZT generates the Reverse links, and ZT reports lost Cards.

ZET departs from Luhmann in **who writes and at what scale**. Agents write the agent Workspace in a fixed form, a whole book per run. So ZET adds Card types, templates, a controlled language, checks, and a hub level above the topic hubs. The user's own thought lives in the human Workspace.

Two gaps remain in practices that ZET accepts:

1. **G1.** The zettelkasten skill still says that ZET cannot insert a Card into a line. `insert_card` can.
2. **G2.** No digest stage looks for Links between books on purpose. The link pass aims at the parts of one book. Only the math digest plans a bridge pass (L6).

## Two systems, two goals

| Aspect | Luhmann's Zettelkasten | ZET card box |
| --- | --- | --- |
| Goal | Produce new thought and publications, with the box as a partner that surprises. | Hold the ideas, models, events, and findings of many books for agents, and the user's own thought for the user. |
| Writers | Luhmann alone, by hand, over about 40 years (more than 90,000 slips). | Two Workspaces. Agents write the agent Workspace in digest runs. The user writes the human Workspace. |
| Unit | One slip: one thought on half a letter sheet. | One Card of a type (Idea, Event, Finding, Model, Hub), with a template for each type. |
| Address | A fixed number that never changes. | A Location that ZT can renumber. ZT rewrites every Link. |
| Order | No order by subject. The place records where a thought first connected. | The same rule for placement. Topic numbers are flat. Models have a computed build order. |
| Text | Own words, few excerpts, the writer's style. | Agent paraphrase of a source, in the Card language (STE). |
| Links | Written by hand, with the concept beside the number. Back references by hand. | Kind words before each Link. ZT computes the Relation kind and the Reverse links. |
| Ways in | The register, collective reference cards, outline cards. | Topic bodies, Hub Cards at four levels (line, topic, field, register), structure Cards. |
| Upkeep | By hand. The register was rewritten four times. | Compilers and checks: `zt-check`, the digest audit, `get_hub`, `get_model`, `list_broken_links`. |

The difference in goal is legitimate. An agent Workspace that grows by whole books needs fixed forms and machine checks that a single writer did not need. The question is where Luhmann's practice still adds value that ZET loses.

## Practice by practice

### Address and filing

| No. | Luhmann's practice | ZET today | Difference |
| --- | --- | --- | --- |
| P1 | Each slip gets a number that never changes (57/12, 57/12a). | A Location changes on a move, an insert, or the deletion of a Side successor. ZT rewrites every Link. | A reference from outside ZET (a printed outline, a note in another tool) can go stale. Links inside ZET stay valid. |
| P2 | No order by subject. A subject hierarchy binds the writer for decades. | Placement files a Card beside the Card it talks to. Topic numbers are flat. A digest opens a Topic per concept, but no Topic sits above another. | Same rule. A Topic title names a subject, but no hierarchy of subjects exists. |
| P3 | A slip continues a line (57/12 → 57/13) or opens a branch (57/12a). | Direct successor and Side successor. | Same mechanism, with a different address grammar (`3/2|c|4`). |
| P4 | A slip goes between two slips (57/12a between 57/12 and 57/13). No number changes. | `insert_card` puts a Card in the place of another. The old Card and its subtree move down one place. | Same result. ZET renumbers and rewrites Links. The zettelkasten skill still says "No insertion" (G1). |
| P5 | A slip never moves. | `move_card` carries a Card with its subtree to another free slot. | ZET can move a misplaced Card. Luhmann gave a misplaced slip a reference. |
| P6 | A new slip can attach to one word of an older slip. | A Link names a whole Card. Math labels (H1, C2) name a clause. | ZET is coarser outside the math units. |

### The unit and its text

| No. | Luhmann's practice | ZET today | Difference |
| --- | --- | --- | --- |
| P7 | One thought on half a letter sheet. | One idea per Idea Card, about 50 to 250 words. A Model Card up to about 600 words. A Model part at most 12 sentences and 400 characters of prose. | Same principle. Math units still split a short proof into many thin Cards (L5 is kept, not built). |
| P8 | One kind of slip. Free form. | Five Card types. Each has a template (for example the six lines of a Finding Card). Model parts have facets. Math units have fixed facets. | ZET adds structure so that agents write the same form and checks can read it. |
| P9 | Own thoughts in own words. Few excerpts. | Agent paraphrase of the book. Math units copy the book's statements faithfully. | The agent Workspace holds the source's ideas, not the reader's. The human Workspace holds the reader's ideas. |
| P10 | Luhmann wrote in his own style. | The Card language: one statement per sentence, short sentences, no semicolons, one term per meaning. `zt-check` checks it. | ZET gives up voice for a single reading. |

### Literature

| No. | Luhmann's practice | ZET today | Difference |
| --- | --- | --- | --- |
| P11 | A separate bibliographic box. Page references on the back of the slip. | A Literature Card per work: BibTeX data and a page index. A Literature tree for the notes bound to the work. | Same split. A Literature tree also holds anchors, Model Cards, and the book's structure Card. |
| P12 | Ideas go into the main box, not under the book (Ahrens's reading of Luhmann). | The book digest files most Idea Cards in Topic trees. The math digest writes no Topic Cards yet. The audit counts each book's Cards by Zone. | Followed in the book digest. Planned for math (L1, L2). |

### Links

| No. | Luhmann's practice | ZET today | Difference |
| --- | --- | --- | --- |
| P13 | A slip is filed once. References record its other places ("multiple storage"). | One home per Card. A Model Card takes in a shared part by a Link. A building block is filed once. | Same principle, applied also to models. |
| P14 | Each reference carries its reason. The concept stands beside the number. | A kind word before each Link. ZT computes the Relation kind. `zt-check` checks L1 to L3. | ZET is stronger: the kind is machine-readable. |
| P15 | Back references by hand, where Luhmann wrote them. | ZT generates the Reverse links on every Card. | ZET is stronger: Reverse links cannot go stale. |
| P16 | The box surprises when it is open and densely, ad hoc connected. Distant Links are the most valuable. | The skill asks for a distant Link at each filing. Topic hubs have a `邻近` group. The digest link pass offers each Card the five most similar Cards of the archive outside its group. But its prompt asks for Links between the parts of one book. A bridge pass between books is planned for math only (L6). | No stage looks for Links to earlier books on purpose, and no report counts them. A book digest can stay an island beside earlier books (G2). |
| P17 | A slip or region that no reference reaches is lost ("black holes"). | The audit lists orphans. `get_hub` lists unreached Cards. `list_broken_links` lists broken Links. | ZET is stronger: ZT reports the black holes. |

### Ways in

| No. | Luhmann's practice | ZET today | Difference |
| --- | --- | --- | --- |
| P18 | A keyword register outside the box. About 1.3 slip numbers per term. Rewritten four times by hand. | The `register` Hub Card, the first Card of the Topic `索引`. One to three hubs per term. The hub compiler rebuilds it. Topic bodies name one to three entry Cards. | Same form. ZET keeps the register inside the box and rebuilds it by script. |
| P19 | Collective reference cards at the head of a section. Up to about 25 references, most into other sections. | `topic` hubs: at most about 25 lines, each with what the Card adds, and a `邻近` group for other trees. | Same form. |
| P20 | Outline cards open a major line of thought. | `line` hubs open one line of thought. A structure Card orders the argument of one book. | Same form. The structure Card serves a book, not the writer's own line. |
| P21 | No hub of hubs. Section numbers from reading history grouped the subjects. | `field` hubs list topic hubs. The register lists field hubs. | ZET adds one level, because its Topic numbers group nothing. |

### Work with the box

| No. | Luhmann's practice | ZET today | Difference |
| --- | --- | --- | --- |
| P22 | Query the box by its link network. The box is a partner in a conversation. | The skill's first step is `search_cards` and a read of the neighbours. A digest writer gets a snapshot of all Topic Cards and can call `search_cards`. | Same principle, done by agents. |
| P23 | A text is not assembled by copying slips. The writer takes slips out, arranges them, and writes fresh. | The Table lays Cards from many trees side by side. A Spread can become a Card. The skill says to write fresh from a structure Card. | Same principle, with a physical-desk GUI. |
| P24 | One writer. Slow growth. The writer feels when a region needs a hub. | Agents write a whole book in one run. Size rules decide when a hub is due. | ZET replaces the feeling with rules. |
| P25 | Later practice (Tietze): collecting is not learning. Process a source before filing. | The skill keeps this rule for hand filing. A digest writes 6 to 12 Cards for each section group of a book. | Digests file the whole book. |

## Proposed changes

### G1. Replace the stale "No insertion" text

**Logic.** P4 is built. `insert_card` puts a Card into a line, and the zettelkasten skill's own model section tells agents to use it. Two texts still say the opposite. An agent that reads them files a mid-line thought as a Side successor when the thought belongs inside the line. `docs/wiki/agent-skills.md` already records this fault.

**Proposal.**

- In `skills/zettelkasten/SKILL.md`, section *ZET addresses*, replace the item "No insertion" with "Insertion renumbers". The new item says: `insert_card` with `before` puts a Card between `3/2` and `3/3`. The old `3/3` becomes `3/4`, and ZT rewrites every Link. Use it for a step that belongs inside the line. Use a Side successor for an example, an objection, or a tangent.
- In `skills/zettelkasten/references/luhmann-and-practice.md`, section *Mapping to ZET*, change the Direct successor item in the same way.
- Add one line to `luhmann-and-practice.md` that names this document for the accepted and rejected practices.

**What it changes.** Two skill files. No ZT change. No Card change. The installed copies of the skill change when the user runs `scripts/install-agent-integrations.ps1`.

### G2. A bridge pass in every digest

**Logic.** Luhmann valued the box for its surprises: one mechanism found in two distant regions (P16). The digest link pass ties the parts of one book together. Its candidate list can hold a Card of another book. But its prompt asks for Links between the parts of the book, and no report counts Links to other books. The hub stage reaches other trees, but a hub only points to Cards. It does not state that two Cards share a mechanism. So each new book can add Cards that do not meet the Cards of earlier books. L6 plans a bridge pass for the math digest only.

**Proposal.**

- Add one bridge stage to the shared digest base, after the link pass and before the hub stage. Then `zt-book-digest`, `zt-econ-digest`, and `zt-math-digest` all run it.
- The stage searches the whole agent Workspace for the key terms of the new book's models, main claims, and events. It proposes Links with a kind word (`对应`, `对照`, `例示`, `依赖`).
- The stage writes a report only. It writes no Link into a Card of an earlier book without the user's go-ahead (the rule of `digest-no-model-reruns`).
- A Link that the user accepts goes into the new book's Card when possible, so that finished Cards of earlier books stay unchanged.

**What it changes.** A new module in the workflow framework's library, a DeepSeek prompt, and a report step. Each installed workflow is frozen, so the change needs a new version of each workflow through the pre-flight agent. No ZT change.

### Already planned: L1, L2, L5, L6, L7

`math-cards-luhmann-comparison.md` keeps these practices for the math cards. They also serve P7, P12, and P16 in this document. This document does not change them.

## Suggested order

| Step | Work | Changes Cards? |
| --- | --- | --- |
| 1 | G1: the two skill texts and the line in `luhmann-and-practice.md`. | No |
| 2 | G2: the bridge module, tried on one book in a pilot, report only. | Only after the user's go-ahead |
| 3 | The math practices L5, L7, then L1 and L2, as the earlier document orders them. | Only in a pilot |

## Questions for the user

| No. | Question | Recommendation | Answer (2026-10-16) |
| --- | --- | --- | --- |
| Q1 | Do you agree with the Accept and Reject list (P1 to P25)? | Yes, as written. | As recommended. |
| Q2 | Should every digest run a bridge pass between books, as a report (G2)? | Yes. Links only after the go-ahead. | As recommended. |
| Q3 | Should a digest keep filing the whole book (P25), or skip sections that give no claim? | Keep the whole book. The audit finds weak Cards. | As recommended. |
| Q4 | Is a whole-Card Link fine-grained enough outside the math units (P6)? | Yes. Split a Card when a Link needs one part of it. | As recommended. |

## References

- Luhmann, N. (1981). Kommunikation mit Zettelkästen. Ein Erfahrungsbericht. Trans. M. Kuehn, "Communicating with Slip Boxes".
- Luhmann, N. (1968). Technik des Zettelkastens. Lecture typescript, Münster, Ms. 2906.
- Schmidt, J. F. K. (2018). Niklas Luhmann's card index: The fabrication of serendipity. *Sociologica*, 12(1).
- Ahrens, S. (2017). *How to Take Smart Notes*.
- Matuschak, A. Evergreen notes should be atomic (notes.andymatuschak.org).
- Tietze, C. The Collector's Fallacy (zettelkasten.de).
- `math-cards-luhmann-comparison.md`, `hub-cards-design.md`, `table-and-spreads-design.md`, `skills/zettelkasten/references/luhmann-and-practice.md`.
