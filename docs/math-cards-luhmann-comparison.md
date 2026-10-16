# Mathematical Cards and Luhmann's Zettelkasten: Comparison

Status: analysis and proposal (2026-10-15), with the user's decisions of 2026-10-15 (see "Decisions"). L4 is built (`639a5a4`). L6 now has source implementation and installed workflows through C2; its real Workflow and accepted live-apply qualification remain open in [the delivery receipt](slip-box-luhmann-delivery-status.md). L1, L2, L5 and L7 remain unimplemented. It compares the math card system (`math-cards-proposal.md` rev. 2, `math-cards-design.md`, branch `math-dev` at `2597c6c`) with Niklas Luhmann's Zettelkasten practice, and it names the practices that can make the math cards better. The text follows ASD-STE100 as the other design documents do.

## Decisions

The user decided these points on 2026-10-15. They replace the recommendations below where the two differ.

**Purpose of the math cards.** The math cards serve the comprehension of AI agents. The user's own comprehension lives in the human Workspace. So no practice in this document asks the reader to write in the reader's own words, and no math feature writes Cards into the human Workspace.

| Item | Decision |
| --- | --- |
| L1 Two layers | Keep |
| L2 One concept, many books | Keep |
| L3 The reader's slip | **Do not apply** (see "Purpose of the math cards") |
| L4 Statement premises and proof premises apart | Keep. Built in `639a5a4`. |
| L5 A short unit stays one Card | Keep |
| L6 Bridges for surprise | Keep |
| L7 The book's number as a stable label | Keep, as a stable label only |
| Luhmann: own words | **Do not apply** |
| Luhmann: literature notes apart from permanent notes | Apply (through L1 and L2) |
| Luhmann: surprise from distant Links | Apply (through L6) |
| Luhmann: half-sheet atomicity | Apply (through L5) |
| Luhmann: no order by subject | Apply: no subject hierarchy decides where a Card is filed. A Card is filed where it first connects. The book's order in the Literature tree and the computed dependency order stay, because they are the order of the source and of the proof, not an order by subject. |
| Luhmann: fixed numbers | Do not apply. ZET keeps its addresses that move, with Links that ZT rewrites. L7 adds only a stable label. |
| Luhmann: one writer, slow growth | Do not apply. Agents write the math cards. |

Nothing in this document goes into the code until the user asks for it.

## Source Authority

- Luhmann's practice: `skills/zettelkasten/references/luhmann-and-practice.md` and the section "What Luhmann did" in `hub-cards-design.md`. They give the evidence (Luhmann 1981 and the 1968 lecture, Schmidt 2016 and 2018, the Bielefeld archive) and the later practice (Ahrens, Matuschak, zettelkasten.de, Doto).
- The math cards: `math-cards-proposal.md` (the reasons) and `math-cards-design.md` (what is built). Code facts come from `src/math.rs` on `math-dev`.
- Mathematical practice outside ZET (the Stacks Project, Lean blueprints, nLab, the mnemonic medium) comes from general knowledge. This analysis did not check those sources again online.

## Short answer

The math card system follows Luhmann well in its **mechanics**: Links that state their reason, one home per unit with references for every other place, a proof as a numbered sequence, inserted steps, and few ways in. It departs from Luhmann in its **purpose and in who does the thinking**. Luhmann's box was a partner for producing new thought. Each slip was a thought he processed in his own words, and the value came from surprising Links between distant regions. The math digest makes a faithful, attention-designed copy of one textbook, written by agents, filed only in the Literature tree. It is a very good study tool. It is not yet a slip-box in Luhmann's sense.

Seven practices can close the gap. Three have a high value (L1 to L3). One is a concrete fault in the built code (L4). The user keeps six of them and does not apply L3.

| No. | Practice | Source of the practice | Value | Cost | Decision |
| --- | --- | --- | --- | --- | --- |
| L1 | Two layers: book units in the Literature tree, and **concept and result Cards in Topic trees** that are not bound to one book | Ahrens (literature and permanent notes), Luhmann's separate bibliographic box | high | medium: one new pass | Keep |
| L2 | **One concept, many books**: a Topic Card per concept links the Definition unit of each book with `对应` and states how they differ | Luhmann's multiple storage, nLab | high | low after L1 | Keep |
| L3 | **The reader's slip**: the Recall station asks the reader to write one sentence in the human Workspace, linked to the unit | Luhmann's communication partner, the generation effect | high | low: GUI only | Do not apply |
| L4 | **Statement premises and proof premises apart** | Lean blueprints (`\uses` in a statement and in a proof) | medium, fixes a fault | low | Keep, built |
| L5 | **A short unit stays one Card** | Luhmann's half-sheet slip, Matuschak on over-splitting | medium | low: architect rule | Keep |
| L6 | **Bridges for surprise**: make the cross-book and cross-domain Link pass (proposal Q6) part of the normal run | Luhmann's surprise, Schmidt's "serendipity" | medium | medium | Keep |
| L7 | **The book's number as a stable label** of each unit | Luhmann's fixed numbers, Stacks Project tags | low | low | Keep |

## Two systems, two goals

| Aspect | Luhmann's Zettelkasten | Math cards |
| --- | --- | --- |
| Goal | Produce new thought and publications. "Communication with a partner that surprises." | Learn and look up the mathematics of one book, with limited attention. |
| Writer | Luhmann alone, by hand, over 40 years (about 90,000 slips). | Agents (DeepSeek writes, Sonnet plans, scripts build). 300 to 600 units per book, each with up to about ten parts. |
| Unit | One slip: one thought on half a letter sheet. | One unit: a Model Card plus its fixed facets (statement, proof plan, proof steps, Derivation parts, intuition, examples, non-examples, hypothesis roles). |
| Order | Fixed number, no order by subject. Placement records where a thought first connected. | Build order and dependency order. A script files each unit as the book numbers it. |
| Text | Own words. Few excerpts. | A faithful statement of the book, plus marked supplements (`（补：书中未给）`). |
| Links | Written by hand, each with its reason. Dense and ad hoc. | Written by agents with kind words. Scripts compute the derived lists (`所用工具`, `后续`, load, used-by). |
| Ways in | Register (1.3 slips per term), collective reference cards, outline cards. | Theory Card (at most 7 main results), Book notation table, hubs, the register hub. |
| Reader | The writer himself. | A learner, with Learn, Review, and Look up modes. |

The difference in goal is legitimate. A mathematics textbook has a correct deductive order, and a learner needs it. So the math cards do not have to copy Luhmann everywhere. The question is where Luhmann's practice adds value that the current design loses.

## Where the math cards already follow Luhmann

| Luhmann's practice | Math card equivalent | Assessment |
| --- | --- | --- |
| A sequence of slips continues an argument (57/12 → 57/13). | A proof is the main line of Direct successors. The titles read as the proof in outline (M15). | Good fit. A proof is the clearest case of a Folgezettel line. |
| A slip can go between two slips (57/12a). | A Derivation part goes between two proof steps with `insert_card`. | Good fit. ZET even rewrites the Links that Luhmann had to fix by hand. |
| A slip is filed once. References give its other places ("multiple storage"). | A building block has one home theory. Other theories list it as a tool. An economics model cites it (D18). | Good fit. |
| References replace re-filing. A slip that needs a new context gets a reference. | A proof cites the statement of a unit and never reaches into its proof (M3, `reach_in`). A shared step becomes a lemma (M11). | Better than Luhmann. It makes a reference mean "I use this statement". |
| Every reference carries its reason. | Kind words (`依赖`, `按定义`, `特例`, `反例`, `对应`), and labels (H1, C2) that name the exact clause used. | Better than Luhmann. A label chip states the reason at the level of one hypothesis. |
| The register points to one or two slips per term. | The Theory Card names at most seven main results. The register hub links a term's Definition unit first. | Good fit. |
| Unconnected slips are lost. | The build report lists units that no Theory Card takes in (`empty`), and hubs list black holes. | Good fit. |
| A slip stands alone. | Look up mode shows a unit by its statement and labels only. | Good fit. A cited unit is a self-contained chunk. |

## Where the math cards depart from Luhmann

| Luhmann's practice | What the math cards do | Effect | Recommended | Decision |
| --- | --- | --- | --- | --- |
| **Own words.** The slip records Luhmann's processed thought. | The digest copies the book's statements and proofs. The reader writes nothing. | The box holds the book, not the reader's understanding. Research on learning agrees with Luhmann here: a learner remembers what the learner generates (the generation effect, Slamecka and Graf 1978) and explains (Chi et al. 1989, already P10 of the proposal). | Change: L1, L3. | **Do not apply.** The math cards serve the agents' comprehension. The user's comprehension lives in the human Workspace. |
| **Literature notes apart from permanent notes** (Ahrens). Ideas go into the main box, not under the book. | The math digest files every unit in the Literature tree (`book/4\|d`). Unlike the book digest, it writes no Topic-tree Cards (`zt-math-digest-workflow.md` names no Topic stage). | The continuity theorem of book A and of book B become two unrelated units. A thought about continuity has no home outside one book. | Change: L1, L2. | Apply (L1, L2). |
| **Surprise from distant Links.** The box is useful when it is open and densely, ad hoc connected. | Links stay mostly inside one theory. Cross-book and cross-domain Links wait for a separate run (Q6). | The math tree can become a closed island. The economics books name the intermediate value theorem but do not reach it. | Change: L6. | Apply (L6). |
| **Half-sheet atomicity.** | A unit is a Model Card plus parts. A lemma with a two-line proof still gets parts. | Many thin Cards. Matuschak and the Obsidian reports warn that over-splitting gives Cards that are titles with almost no body. | Change: L5. | Apply (L5). |
| **No order by subject.** Placement records history. | The skeleton files units in book order. Dependency order decides the study path. | A deductive subject needs a dependency order. Luhmann's objection was to a fixed topical hierarchy, not to the order inside one argument. | Accept. | Apply: no subject hierarchy decides where a Card is filed. A Card is filed where it first connects. The book's order and the dependency order stay. |
| **Fixed numbers.** A slip number never changes. | ZET addresses shift when a Side successor is deleted or a Card moves. ZT rewrites every Link. | Links inside ZET stay correct. A reference from outside ZET (the reader's own notes, a printed outline) can go stale. | Small change: L7. | Do not apply fixed numbers. Keep L7 as a stable label. |
| **One writer, slow growth.** | Agents write a whole book in one run. | Luhmann's "squeeze point" feeling does not exist. The hub design already replaces it with size rules. | Accept, with L3 to give the reader a part in the writing. | Do not apply. Agents write the math cards. L3 is not applied. |

## The recommended practices

### L1. Two layers: book units and Topic Cards

**Logic.** Luhmann kept the bibliographic box apart from the main box. Ahrens names the two kinds: a literature note records what a source says, with its page. A permanent note records an idea the writer would defend without the source. ZET already has this split (Literature trees and Topic trees), and the book digest uses it. The math digest uses only the first layer. A theorem is not bound to the book that proves it. The intermediate value theorem is the same theorem in every analysis book and in every economics model that uses it.

**Proposal.**

- The Literature tree keeps what is built: one unit per numbered environment, faithful to the book, with the attention design. This stays the study layer.
- A new, small **Topic pass** at the end of the math run writes Topic Cards for the theory objects and the main results only (the units that a Theory Card lists in `对象` and `主要结果`). That is at most about 7 + 5 Cards per chapter, not one per unit.
- Each Topic Card is an Idea Card in the zettelkasten skill's sense: a claim title, 50 to 250 words, no proof, written by the agents under the Card rules of the book digest. (The user's own-words practice is not applied: see "Decisions".) It says what the result means, why it matters, and where it is used. It links the book unit with `对应` (or `依据`), and other Topic Cards with kind words.
- Proofs, examples, and study stations stay in the Literature tree. The Topic Card does not repeat them (the rule of M16, at the level of two layers).

**What it changes.** A new pass in the math workflow and its prompt supplement. No ZT change. The live archive gets new Topic Cards only when the user runs a digest.

- Placement (no order by subject): the pass files each Topic Card where it first connects, as a successor of the Topic Card it continues or in the Topic tree where its subject already starts. No subject hierarchy of Topics (such as analysis, then continuity) decides the place. A hub gives the ways in.

**Open choice.** Should the Topic Card be a Model Card with a Math form (so that `get_model` reports it), or a plain Idea Card? An Idea Card is simpler and keeps the Math form for the faithful unit. This analysis recommends the plain Idea Card. The user has not decided this yet (C2).

### L2. One concept, many books

**Logic.** Luhmann solved "multiple storage" with one slip and many references. The math design gives a concept one home per book. A second analysis book makes a second continuity unit, with a different definition (ε-δ, or sequences). The proposal's answer is a `对应` Link between the two Definition units (link network table). That Link has no home for the comparison itself. nLab and the Stacks Project keep one page per concept and list its equivalent definitions on that page. That is a hub with reasons, in Luhmann's terms a collective reference card.

**Proposal.** The Topic Card of a concept (L1) is the one place where the books meet. It lists each book's Definition unit, one line each, and says how the definitions differ and which result makes them equivalent (`等价 [[…]]`). The register hub links this Topic Card first, then the Definition units. When only one book defines the concept, the Topic Card links one unit.

**What it changes.** A rule in the Topic pass of L1, and one line in the hub stage's register rule (`math-cards-proposal.md`, "Concepts and hubs"). No ZT change.

### L3. The reader's slip

**Decision: not applied.** The math cards serve the agents' comprehension, and the user keeps their own comprehension in the human Workspace. The text below stays as the record of the proposal only.

**Logic.** The math cards design study (Learn, Review, Recall) but keep the reader passive in the slip-box. Luhmann's box answered back because its writer had put his own thought in. A reader who writes one sentence in their own words, at the moment of recall, both learns more (generation effect, self-explanation) and makes the box their partner. Proposal question Q7 asks whether ZT should record what a reader studied. A Card is a better record than a hidden database: the reader can read it, link it, and find it again.

**Proposal.**

- At the Recall station of a unit or a theory, the study desk offers a field: "State it in your own words" or "Why does H2 matter?".
- When the reader writes something, the GUI creates an Idea Card in the **human** Workspace (the reader's own box), with the unit Link in its body (`回忆 [[book/4|d]]：…`). The unit's Reverse links then show the reader's slips.
- The study path can then skip a unit that has a recent reader's slip, which answers Q7 without a new table.
- Nothing is written without the reader's action. An agent never writes into the human Workspace.

**What it changes.** The GUI (study desk) and one GUI endpoint that creates a human Card. ZT's rule that agents do not write human Cards stays. This needs a decision by the user, because it is the first feature that writes the user's own Workspace from the GUI.

### L4. Statement premises and proof premises apart

**Logic.** A Lean blueprint (Massot's `leanblueprint`, used by Tao and others) records dependencies twice: `\uses` in a statement (what a reader needs to understand it) and `\uses` in a proof (what a reader needs to verify it). The dependency graph colours the two kinds. The math design has the same two needs. Look up mode and the Meet pass need the statement's premises. The Why pass needs the proof's premises. D15 says that a lemma comes just in time, at the step that cites it.

**The fault in the built code.** `src/math.rs:842` collects the premises of a unit from all its Cards (the Model Card and every proof part) into one set. The theory study path (`src/math.rs:1019` to `1037`) then walks this set from each main result, through lemmas, and puts every unit it finds before the main result, except the lemmas themselves. So a definition that only a lemma's proof uses comes **before** the main result, although the lemma itself comes just in time. The reader meets a concept long before the step that gives it a motive. This goes against D15 and against principle P12 of the proposal.

**Proposal.**

- In `premises_of`, keep two sets: the premises of the Model Card (statement premises) and the premises of the parts (proof premises).
- The prerequisite path of a main result holds its statement premises, transitively, and the proof premises of the main result itself. It does not follow the proof premises of a lemma. Those come with the lemma, just in time.
- The dependency order and the cycle check keep the union, as now.
- `get_model` reports both sets, so the theory board can draw a proof premise as a thinner thread.

**What it changes.** `src/math.rs` and its tests. No storage change. This is the only recommendation that fixes a fault in what is built.

**Status: built on `math-dev` (not committed).** The test `a_prerequisite_path_follows_statements_and_leaves_a_lemmas_proof_for_just_in_time` in `tests/math_units.rs` showed the fault first. The theory report now keeps `premises` (all premises: order, cycles, forward citations, tools) and `statement_premises` (the Model Card and its `前提` parts). The path follows a lemma by its statement premises only. Each unit in `order` reports `statement_cites` beside `cites`. The theory board does not draw the two kinds apart yet.

### L5. A short unit stays one Card

**Logic.** Luhmann's slip was half a letter sheet. The zettelkasten skill takes about 50 to 250 words per Card. A corollary with a two-line proof, or a definition with one clause, fits in one Card. Splitting it into a Model Card, a `前提` part, a `证明：…` part, and an `例子` part gives four thin Cards and four Links to keep. Matuschak warns that over-splitting breaks the link network. The math design already drops the `证明思路` part for a proof of at most six steps.

**Proposal.** A unit at the `concept` level, and a lemma or corollary whose proof has at most about three sentences, stays one Card: statement, proof, and one example in the Model Card body, under the bold labels `陈述`, `证明`, `例子`. The architect's math supplement and `math-compiler.ps1 skeleton` apply it. The study desk reads the bold labels already (design, "Statement and motive"), so it can show the sections as stations without separate Cards. The attention budget M13 then counts sections, not parts.

**What it changes.** The architect supplement, the skeleton step, and the study station code for one-Card units. The pilot (Q4) measures how many units this affects before the user decides.

### L6. Bridges for surprise (carried out by C2 of slip-box-luhmann-design.md, math profile)

**Logic.** Luhmann valued the box for its surprises: the same mechanism found in two distant regions. Schmidt calls the box a generator of serendipity. In the math design, the most surprising Links are the ones between mathematics and the economics books (D18, Q6) and between theories of different books. The proposal leaves them to a later, separate run.

**Proposal.**

- Run a bridge pass in every math digest after the link pass. It searches the whole agent Workspace for the names of the book's main results and objects (the theorem's name, the concept's term), and proposes `应用` or `对应` Links. The proposal goes to the user as a list. The pass writes nothing into finished economics Cards without the user's go-ahead (the rule of `digest-no-model-reruns`).
- The Theory Card's `后续` list then shows economics models as users, and the load of a unit counts them.

**What it changes.** A new stage in the math workflow, with a report and a confirm step. The `应用` word is still not a rests-on word (the design's scan found that it would change finished models). The bridge pass therefore writes `依赖` or `基于` before the Link, as the design requires.

### L7. The book's number as a stable label

**Logic.** Luhmann's numbers never changed, so a reference from anywhere stayed valid. The Stacks Project gives each result a permanent tag for the same reason. ZET rewrites Links inside the archive, so Links stay correct. The book's own number (`定理 4.5.1`) is the stable name a reader uses outside ZET. Today it stands only in the `来源` line.

**Proposal.** Keep the book's number in the unit's title prefix or in a fixed `编号：定理 4.5.1` line, and let `search_cards` find a unit by it. A later option is an alias address (`[[book#定理4.5.1]]`), but that is a ZT change and has a low value while Links are rewritten.

**What it changes.** One line in the skeleton step. No ZT change for the first step.

## What not to take from Luhmann

- **Fixed numbers for every Card.** ZET's moving addresses with rewritten Links are better inside the archive. L7 keeps only a stable label.
- **No order at all.** A proof and a theory have a deductive order, and a learner needs it. The dependency order stays.
- **One writer only.** The agents' scale is the reason the math digest exists. The agents write the math cards.
- **Own words of the reader.** The math cards serve the agents' comprehension. The user's own comprehension lives in the human Workspace (user's decision, 2026-10-15).
- **Hand-written reasons only.** The computed lists (`所用工具`, `后续`, load) are an improvement, because they cannot go stale as Luhmann's register did.

## Suggested order

| Step | Work | Changes Cards? |
| --- | --- | --- |
| 1 | L4: the test that shows the fault, then the fix in `src/math.rs`. | No |
| 2 | L5 and L7: rules in the architect supplement and the skeleton. Measure in the pilot. | Only in the pilot |
| 3 | L1 and L2: the Topic pass and its prompt, tried in the pilot. | A new pilot tree and its Topic Cards |
| 4 | L6: the bridge pass, report only, then the user's go-ahead (carried out by C2 of slip-box-luhmann-design.md, math profile). | Only after the go-ahead |

Step 1 is built (`639a5a4`). Steps 2 to 4 are kept and wait for the user to ask for them. L3 is not applied, so it has no step.

## Questions for the user

| No. | Question | Recommendation | Answer (2026-10-15) |
| --- | --- | --- | --- |
| C1 | Should the math digest write Topic Cards for the main results and objects (L1, L2)? | Yes, for main results and theory objects only. | Yes (L1 and L2 kept). |
| C2 | Should a Topic Card be a plain Idea Card or a Model Card with a Math form? | A plain Idea Card. | Open. |
| C3 | May the study desk create Cards in the human Workspace when the reader writes (L3)? | Yes, only on the reader's action. | No (L3 not applied). |
| C4 | Should the bridge to the economics books run in every math digest, as a report (L6)? | Yes, as a report. Links only after the go-ahead. | Yes (L6 kept). |
| C5 | Should a short unit stay one Card (L5)? | Yes, after the pilot shows how many units it affects. | Yes (L5 kept). |

## References

- Luhmann, N. (1981). Kommunikation mit Zettelkästen. Ein Erfahrungsbericht. Trans. M. Kuehn, "Communicating with Slip Boxes".
- Schmidt, J. F. K. (2018). Niklas Luhmann's card index: The fabrication of serendipity. *Sociologica*, 12(1).
- Ahrens, S. (2017). *How to Take Smart Notes*.
- Matuschak, A. Evergreen notes; Evergreen notes should be atomic (notes.andymatuschak.org).
- Matuschak, A., and Nielsen, M. (2019). *Quantum Country* and the mnemonic medium.
- Slamecka, N. J., and Graf, P. (1978). The generation effect. *Journal of Experimental Psychology: Human Learning and Memory*, 4(6), 592–604.
- Chi, M. T. H., et al. (1989). Self-explanations. *Cognitive Science*, 13(2), 145–182.
- The Stacks Project: permanent tags and dependency graphs (stacks.math.columbia.edu).
- Massot, P. `leanblueprint`: statement and proof dependencies in a blueprint graph.
- nLab: one page per concept, with its equivalent definitions (ncatlab.org).
