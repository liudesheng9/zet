# Co-Design Discussion Log: Multi-Tree Coordinates in ZET (Gemini ↔ Astra)

- **Date:** 2026-11-24
- **Repository:** `E:\zet` (working checkout `06cb56c8`)
- **Participants:** Gemini (Gemini 4 Neon, High) & Astra (Codex `gpt-6.2-astra`, reasoning effort `high`, standard service tier, read-only sandbox)
- **Astra Session ID:** `01a20c4b-358b-78c1-b876-0c4635dfd142`
- **Resulting Proposals:**
  - English: [multi-tree-coordinates-proposal.md](multi-tree-coordinates-proposal.md)
  - Chinese: [multi-tree-coordinates-proposal.zh.md](multi-tree-coordinates-proposal.zh.md)

This file preserves the full verbatim three-round architectural discussion between Gemini and Astra, followed by the post-Round-2 code verification note.

---

## Round 1 — Gemini's Opening Brief to Astra

# Co-Design Request: Multi-Tree Coordinates & Richer Card Organization in ZET

You are co-designing a major architectural evolution of the `zet` (`ZT`) note system at `E:\zet` together with me (Gemini). The user explicitly asked the two of us to **fully discuss** this design and produce a thorough, logically consistent Markdown proposal grounded in **first principles of human comprehension** and ZET's actual architecture.

This is **Round 1** of our discussion. Please inspect the relevant parts of the `E:\zet` codebase (`CONTEXT.md`, `docs/wiki/README.md`, `src/main.rs`, `src/ops.rs`, `src/place.rs`, `src/link.rs`, `src/model.rs`, `src/math.rs`, `src/hub.rs`, `src/tree_lock.rs`, `src/gui.rs`, etc. — whatever you need to ground your analysis in the real code) and give a deep, opinionated, first-principles architectural critique and design proposal. In Round 2, I will challenge, synthesize, and stress-test your ideas against mine until we resolve every edge case and reach full coherence.

## The User's Exact Motivation

> "motivation: want to have more organize relationship between cards. other than literature tree, there should be other trees. one card can have coord in one tree, and can have coord for another tree.
> eg: multiple literature tree, and their root belongs to anothor tree, this tree is for a lietrature group for checking the relationship between them or they concern one problems.
> eg2: math lit, one thm can link to another defs, and reader can learn this relation ship more clearly.
> eg3: evidence of one history thread is separated in different history lits, so use a tree to link them and easy fro user to check.
> I want you and astra to fully discuss on this and make sure that the logic is consistent/following the 1st principls of human comprehension. compose the output proposal into markdown version."

## Core Design Questions We Must Resolve Together

Please examine the codebase and address these questions thoroughly (and raise any deeper questions I missed):

1. **First Principles of Human Comprehension (Why Trees vs. Graphs vs. Hubs vs. Spreads?):**
   - What cognitive job does a **Folgezettel tree** (Direct spine + Side branches) perform that inline `[[links]]`, Hub Cards, Theory Cards, Model boards, and Spreads currently do *not* perform well?
   - Conversely, where does forcing a relationship into a tree *hurt* comprehension (e.g., DAGs like math theorem → multiple definitions in Example 2)? How should Example 1 (literature-group/problem tree), Example 2 (math theorem ↔ definitions comprehension), and Example 3 (cross-literature historical evidence thread) each be served without blurring ontological boundaries?
   - Should a Card be able to appear in **multiple trees**, AND can it appear **more than once in the same tree** (e.g., a definition needed in two places of a learning tree, or a paper cited in two branches), or strictly **at most once per tree**? What are the cognitive and structural tradeoffs?

2. **Card Identity vs. Home Coordinate vs. Secondary Placements:**
   - Today in ZET, `cards.location` / `cards.citation_key` fuses **identity**, **ownership/Workspace/Lock**, and **tree coordinate** into one column.
   - If a Card can hold coordinates in multiple trees:
     - Should every Card still have **one canonical home tree** (e.g., its Literature tree or originating Topic tree) plus **0..N secondary placements** in other trees — OR should all tree coordinates be symmetric placements over an internal immutable `card_id`?
     - How should `[[...]]` Links work? Must Links point to a canonical Card address (or stable ID), or can a Link target a specific tree coordinate (context-qualified)? What happens to link rewriting, Reverse links, and human-workspace read-only protection?
     - What is the exact lifecycle of **edit**, **move**, **unplace/detach**, and **delete**? (Specifically: removing `Hart1995` from a Literature-Group tree must NOT delete Hart's Literature tree; deleting a home card must cleanly handle or warn about secondary placements!)

3. **Edge / Placement Context (Why is this Card at *this* coordinate in *this* tree?):**
   - In Example 1, `Hart1995` sits as a Direct successor of `Grossman1986` in a Literature-Group tree. In Example 3, an Event Card from History Book B sits under an Event Card from History Book A in a History-Thread tree.
   - Where does the **tree-local rationale / relation** live (e.g. "extends GHM to multi-asset authority", or "causal consequence", or relation kind `rests-on`/`contrasts`/`illustrates`), given that the underlying Card may be in a **Locked** tree or the **human** Workspace and shouldn't be polluted by every tree that includes it?
   - Can a secondary tree mix **mounted/placed existing Cards** AND **tree-native Cards** (e.g., synthesis claims, transition notes, section anchors written specifically for that tree)?

4. **Taxonomy of Trees (Zones / Tree Kinds) & Relation to Existing ZET Concepts:**
   - Today ZET has `Topic` trees (`<n>/0`) and `Literature` trees (`<CitationKey>`).
   - What tree kinds / zones should exist after this update? Should we keep the taxonomy minimal and orthogonal (e.g., distinguish by *role* rather than domain-specific hardcoding like `history-tree` vs `math-tree`), OR should trees carry a lightweight kind/purpose?
   - How does this interact with — or unify/simplify — existing **Topic trees**, **Hub Cards** (`line`/`topic`/`field`/`register`), **Theory Cards**, **Model imports (`**引入**`)**, and **Spreads**? (Especially: what happens to Hub Cards and `zt-idea-grow` / `zt-post-digest`?)

5. **Data Model, Address Grammar, Invariants, and Subsystems:**
   - What concrete SQLite schema changes make this rock-solid, zero-data-loss, and clean to migrate from existing archives?
   - What should the **coordinate grammar** look like so both humans (in Terminal Session, CLI, Markdown) and agents (via MCP) find it unmistakable and ergonomic?
   - How do **Workspaces (`human` vs `agent`)**, **Tree Locks & Pins**, **Model/Math/Term/Cited structural computation** (which currently walk `parent_location`!), **Write Gate / `zt-check`**, **Link Index**, **Dump (`zt dp`)**, and **Portable Export (`zt export`)** behave under multi-tree coordinates?

6. **Human Comprehension UX (GUI + Session + MCP):**
   - How should the **GUI** (Cabinet, Folgezettel map, Reader, Breadcrumbs, "Also in trees" / coordinate switcher, Context-aware navigation, Subtree zoom for mounted Literature roots, Math dependency lens vs. authored tree) make multi-tree comprehension *effortless* rather than confusing?
   - How should the **Terminal Session** (`Pointer` = `(card, tree_coord)`!) and **MCP tools** work?

## Output Format for Round 1

Please structure your Round 1 response as a clear, candid architectural memo covering:
1. Your core thesis & first-principles diagnosis of ZET's current bottleneck.
2. Your proposed conceptual model & ontology (with explicit positions on every hard choice above).
3. How all three user examples (Lit Group, Math Thm↔Defs, History Evidence Thread) work concretely end-to-end under your design.
4. Schema, address grammar, invariants (Locks, Workspaces, Models/Terms/Hubs), and GUI/Session/MCP implications.
5. Open tensions, trade-offs, and specific questions you want me to push back on in Round 2.

---

## Round 1 — Astra's Response (`gpt-6.2-astra`, high)

# Round 1 design memo — One Card, several reading trees

**I would give every Card one home and allow it to occupy one position in each of any number of additional trees.** I would introduce one new tree kind, a **Reading tree**, which can interleave existing Cards with Cards written for its question. Each placement can explain why the reader encounters this Card here. A Literature root can occupy position two in a Reading tree while remaining the root of its entire book.

The test case for this architecture is simple. A reader opens Hart from a comparison of papers, reads three Cards inside Hart’s book, and goes Back. They should return to the comparison at Hart, see the editor’s reason for putting it after Grossman, and continue to the next paper. Removing Hart from that comparison should leave the book intact. If those operations are awkward or dangerous, the data model has assigned the wrong meaning to containment.

This is a Round 1 proposal based on the working checkout at `06cb56c8`, including its local workflow documentation changes. The workspace is read-only, so the Markdown memo is delivered here. The examples below illustrate proposed behavior, not records verified in the user’s archive.

## 1. What the reader needs from a tree

ZET already answers “What else is connected to this Card?” quite well. Its missing operation is **“Show me how someone who understands this question would lead me through material filed in several places, and let me inspect their choices.”**

That distinction matters because reading involves different decisions at different times.

| Reader’s question | Representation that serves it | What the reader gets |
|---|---|---|
| What should I read next? Where can I take a detour and resume? | Folgezettel tree | One Direct continuation, identifiable Side alternatives, and a recoverable position |
| What does this claim depend on? What specifically disagrees with it? | Authored Links and a focused graph | Several independently justified relationships, including relationships to Cards already encountered |
| Where can I enter this subject? | Hub or Register | Named ways in, grouped by the reader’s interest or vocabulary |
| Which premises support this result, and which proof step uses them? | Theory or Model board | Computed membership, dependency order, and the authored basis for an edge |
| What do these accounts say side by side? | Table and Spread | Simultaneous inspection with an arrangement the reader can keep |

There is a cognitive reason to preserve these distinctions. A representation earns its place when it makes a recurring operation easy. A tree saves the reader from choosing among all neighboring Cards at every step. A graph saves the reader from reconstructing a many-to-many relationship from a sequence. A Spread saves the reader from remembering one passage while inspecting another. Larkin and Simon’s analysis supports this narrower principle: arranging related information together can reduce search and make an inference explicit. Applying that principle to ZET is a design inference. [Larkin and Simon, *Why a Diagram Is (Sometimes) Worth Ten Thousand Words*](https://doi.org/10.1111/j.1551-6708.1987.tb00863.x)

**Direct should mean “continue this reading.” Side should mean “pursue this alternative from here.”** Neither is sufficient evidence that the later paper extends the earlier paper, that one historical event caused the next, or that a theorem requires the preceding definition. Those are separate statements with an author and a basis. The GUI should let a reader see that basis exactly where the sequence invites the inference.

This also explains why a better Hub alone would only partly satisfy the request. A Hub already supports a grouped list with a sentence about each member. For three papers with three independent descriptions, I would use the Hub. A Reading tree becomes worthwhile when the reader needs a **particular predecessor, a continuation, a branch point, or a place to resume**. Those four things should be the visible payoff for authoring it. [Hub parsing and range reports](E:/zet/src/hub.rs:186) · [Spread storage and behavior](E:/zet/src/spread.rs:1) · [ZET’s Folgezettel and Table vocabulary](E:/zet/CONTEXT.md:183)

## 2. Identity, home, placement, and rationale

One correction to the premise affects the design. ZET already has an integer `cards.row_id` primary key and a separate `workspace` column. A Literature root has a Citation key and BibTeX. Other Cards have a Location. The coupling arises because the ordinary read path exposes `COALESCE(location, citation_key)` as the address, and operations infer root, parent, zone, and frequently meaning from that address. The Lock subsystem already distinguishes a Card’s row identity from its address. There is no need to discard either distinction to support multi-tree reading. [Card schema](E:/zet/src/main.rs:1634) · [Card loading and Location arithmetic](E:/zet/src/main.rs:2859) · [Lock members and pins](E:/zet/src/tree_lock.rs:1433)

I propose four concepts.

| Concept | Meaning | Example |
|---|---|---|
| **Card** | One authored text and metadata record with immutable identity and one owning Workspace. | Hart’s bibliographic record and its Markdown body |
| **Home** | The one structural filing used for editing, source organization, and rules that depend on filing. Its address remains the ordinary way to cite the Card. | `Hart1995` at the root of its Literature tree |
| **Placement** | One identified occurrence of a Card in an authored tree. Its parent, Direct or Side role, and order belong to that tree. | Hart at `~7/2` in a Reading tree about ownership |
| **Placement rationale** | The tree editor’s account of why this Card follows or branches from this particular predecessor. | “Read Hart here to compare how the two accounts allocate control.” |

**Keep one canonical home.** A Term part needs one answer to “Which definition am I continuing?” A Literature Card needs one answer to “Where does my source begin?” An agent needs one answer to “Who owns this text?” Symmetric coordinates would force each caller to supply a privileged context before it could answer these ordinary questions. That would quietly recreate the home concept in several subsystems.

**Permit one placement per Card per tree, including the home placement.** A reader can then ask “Where is this definition in this reading?” and receive one answer. A search result can offer one destination per tree. An editor who tries to place it again should see its existing coordinate and two actions: **go there** or **refer to it here**. A short recall cue such as “Revisit the definition of compactness before reading this step” may link to it anywhere. A distinct Card is justified when the author has something distinct to say, such as why compactness matters in this argument. The cost is real: a course author cannot give the same text two numbered stations. I would accept that cost provisionally and test it in Round 2.

**Give each Reading tree a question and an owner.** “How do these works explain control over assets?” tells a reader what the route selects for. A mutable title and optional purpose such as `comparison`, `learning`, or `chronology` help the Cabinet find it. Purpose carries no permissions or inference rules. The existing Literature type already serves as a shelf label for works. The archive needs one new organizational kind, `reading`, alongside `topic` and `literature`; it does not need a schema migration each time someone invents a new use for a tree. [Current Zone, Workspace, and Literature type definitions](E:/zet/CONTEXT.md:90)

**Allow both mounted and native Cards.** “Mount” means placing one existing Card by identity. Its home descendants stay at home. A native Card is written for this Reading tree and has its home here. The root is a native Card stating the question. Native synthesis claims, transitions, and section anchors give a route its own intellectual content. In the first release I would support ordinary prose Cards for these purposes and keep creation of structurally specialized families in their existing home environments. A whole Model, Term head, Cited Card, Hub, or Literature root can still be mounted.

**Attach the rationale to the incoming placement edge.** Store a short sentence and, optionally, a structured relation kind with a fixed direction, **current Card relative to predecessor**. The UI should render both names, for example “Hart extends Grossman,” even though the reader travels Grossman → Hart. Reuse a defined relation kind when it fits. A chronology can simply say “The next dated account comes from Book B.” `Direct` plus `rests-on` must never manufacture a premise in a Model report. For the first release I would allow plain text in rationales and keep substantial, sourced, or independently citable arguments in native Cards with ordinary Links.

One consequence deserves emphasis. **Changing the parent changes the proposition expressed by an edge.** If Hart is moved after a different author, the old “extends Grossman” sentence cannot silently remain attached to the new edge. A move preview must list *every* changed predecessor, including the Card left behind at the old position and any Card displaced at the new one. Require a replacement rationale or explicit clearing for each affected nonempty rationale. Retain the old sentence in the preview. Simple renumbering that preserves both endpoints preserves the sentence. This is an intellectual integrity rule as important as maintaining foreign keys.

## 3. Three end-to-end examples

### A. A group of works organized around one problem

Suppose a reader creates a human Reading tree with the question “How do theories of ownership explain who controls an asset?” The coordinates below are illustrative.

| Coordinate | Card | Why it is here |
|---|---|---|
| `~7/0` | Native root stating the question | Sets the scope and criterion for comparing works |
| `~7/1` | Mounted Literature root `Grossman1986` | Introduces the baseline account selected by the editor |
| `~7/2` | Mounted Literature root `Hart1995` | Invites comparison of which control rights each account assigns |
| `~7/2\|a` | Native synthesis Card | States the editor’s specific comparison and cites the passages supporting it |
| `~7/3` | Another mounted work | Continues the main reading under a stated reason |

At `~7/2`, **Continue** follows `~7/3`. **Open book** enters the home tree at `Hart1995`. A third action, **Compare passages**, can put the selected Cards on the Table. Back from the book restores `~7/2`, its scroll position, and its branch state. A hundred new Cards in Hart’s digest add zero nodes to the comparison.

If the reader removes Hart, the preview shows the fate of the synthesis branch and the changed predecessor of `~7/3`. It permits an explicit reattachment of the synthesis. The only Hart object being removed is the placement. Deleting Hart’s Literature tree is a different command with a different target and impact preview.

### B. A theorem that uses two definitions

Suppose theorem `T` uses definitions `D1` and `D2`, and theorem `U` also uses `D1`. The primary view for the question “What does the proof use?” should show both premises of `T`, the shared use by `U`, and the sentences or proof steps that supply those relationships. It should distinguish an authored `rests-on` premise from a vocabulary Term link. The latter can explain a word without claiming that the proof invokes the linked Card as a premise.

ZET already has unusually good machinery here. `theory_report` collects the units named by a Theory Card, computes premises across the statement and its parts, separately computes statement premises for reading, and orders ready units with Folgezettel order as the tie-break. It also handles cyclic or blocked orderings. I would expose that evidence in a focused dependency lens before building any new math edge store. When a needed relationship is absent from the source, the lens should show what evidence is available. An editor can record their interpretation in a native Card, including when the source is Locked. [Theory report](E:/zet/src/math.rs:834) · [Model imports and closure](E:/zet/src/model.rs:945) · [Term-link semantics](E:/zet/CONTEXT.md)

A teacher can separately author the sequence `D1 → example → D2 → T → U`. At `U`, “Recall D1” opens the same definition and returns to `U`. The reader can change study order without changing prerequisite closure. Conversely, correcting a documented premise updates the dependency lens without silently rearranging the teacher’s route. The visible distinction is between **“Read next”** and **“Uses in the proof.”**

### C. Evidence for a historical thread dispersed across books

Take two Event Cards. Book A reports an order on a given date. Book B reports a later change in local practice. Mount both in date order and show each Card’s book, source locator where recorded, and authored account. The second placement can say “This account supplies the next dated observation.” The route now serves the basic request: a reader can check the thread across books in order.

The stronger proposition, “The order caused the change,” deserves a native synthesis Card. It can cite both Events, identify what each source actually observed, and state what supports the causal interpretation. A conflicting account can be a Side branch from the point of disagreement. Two books drawing on the same underlying document can remain two source accounts, while the synthesis records their dependence. Counting two placements as two independent observations would be a substantive error.

This design gives the reader three distinct actions at the same point: continue the chronology, inspect the evidence at home, or follow the historian’s inference. Editing the chronology changes the first. Revising a source Card changes the second everywhere it appears. Revising the causal argument changes the third.

## 4. Storage and address design

### Write one new structure first

I would **keep the existing Topic and Literature home representation for the first release**. Put it behind a typed home-tree reader. Add explicit placement rows for Reading trees. Both readers can answer “root, parent, Direct child, Side children, coordinate”; only the home reader supplies structural context to existing Card rules. This saves a full archive rewrite simply to display the first cross-book comparison.

A concrete target schema follows. These are logical table definitions. Legacy `cards` and `locks` retain their current keys and constraints except for the explicitly versioned extension for Reading-native Cards.

| Table or extension | Authoritative fields and constraints |
|---|---|
| `card_identity` | `card_id` immutable UUID primary key, `card_row` unique FK to `cards.row_id`. Allocate once in the Card-create transaction. Use the UUID across dumps, sessions, and placement references. Preserve existing row IDs for Lock evidence. |
| `reading_trees` | Immutable `tree_id`, unique never-reused display number, owning Workspace, root Card ID, optional purpose. The root Card supplies the title and question. |
| `reading_placements` | Immutable `placement_id`, `tree_id`, `card_id`, nullable `parent_id`, `edge` in `root/direct/side`, Side ordinal where applicable, rationale, nullable relation kind. Unique `(tree_id, card_id)`, one root per tree, one Direct child per parent, unique Side ordinal per parent. |
| `reading_homes` | `card_id` primary key, `tree_id`, immutable local Card number unique within the tree. A deferred composite reference requires this native Card to have a placement in its owning tree. Existing Topic and Literature homes remain in `cards`. |
| Reading-tree Lock evidence | Versioned placement-member records containing endpoints, order, rationale fingerprint, and target identity. Identify pins originating at placements separately from pins originating in Card bodies. Historical evidence survives deletion after unlock. |

Use a composite foreign key `(tree_id, parent_id)` to a placement in the same tree. Enforce one connected acyclic tree, one native root, no Side child of the root, positive Side ordinals, valid edge fields, exactly one home per Card, and identical Workspace for a native Card and its home tree. A mounted Card may belong to either Workspace. Foreign keys should restrict deletion of referenced targets. Validate reachability and root coherence inside the write transaction, including batch operations. Add indexes for lookup by target Card and ordered children. Derive displayed coordinates from the ordered structure; a coordinate cache must be reconstructible.

There is one deliberate departure from legacy filing. A Reading-native Card can sit between two mounts. Its canonical address should therefore survive rearrangement of the reading. Give it an immutable local Card number. Extend the stored Card-kind/address checks explicitly, including the distinction between a Reading root and a Literature root. Today, code such as `ops::zone` treats a non-Literature tree as Topic. Every such two-way dispatch needs an exhaustive three-way replacement before enabling the new writer. [Current tree grammar](E:/zet/src/main.rs:2705) · [Kind and zone dispatch](E:/zet/src/ops.rs:1692) · [Placement planner](E:/zet/src/place.rs:1)

### Give humans two short spellings with different meanings

| Spelling | Meaning |
|---|---|
| `[[Hart1995]]`, `[[3/2\|a]]` | Existing authored Card Links. Their syntax and home-address meaning continue. |
| `~7/2`, `~7/2\|a` | Current positions in Reading tree 7. Accepted by navigation and placement commands. |
| `[[~7/c4]]` | The fourth locally numbered native Card in Reading tree 7. Its number survives reordering. Root uses `c0`. |
| `zt-place:<archive-id>/<placement-id>` | Durable navigation bookmark for this occurrence, usable as a Markdown destination or by the Session. It resolves after a reorder. Detaching the placement retires the bookmark. |

The `~` prefix is available because Citation keys begin with an ASCII letter and Topic IDs are numeric. Reserve it in the shared parser and give it parser tests across CLI, Markdown, MCP, GUI, and chart links. Keep `@` for the existing labelled Term syntax. Quote coordinates containing `|` in a shell command. [Citation-key validation](E:/zet/src/literature.rs:132) · [Link validation](E:/zet/src/link.rs:226)

**Do not accept `[[~7/2]]` as a Card Link.** If the route is reordered tomorrow, the same text could identify a different paper. From a placement, **Copy Card link** returns `[[Hart1995]]`; **Copy place** returns the durable occurrence bookmark. A manually typed coordinate addresses today’s slot. A bookmark follows one occurrence. A stale bookmark reports that its placement was removed, even if a new placement occupies the old slot.

Authored Card Links can continue using readable canonical addresses. At resolution time the system can return the immutable Card ID. A legacy home move or Citation-key rename still runs the existing authored-Link and chart rewrite protections. Placement references follow the Card ID with no Markdown rewrites. Preserve the rule that MCP refuses a rename requiring edits to a human Card body. Preserve current Broken-link behavior for old textual references. The new immutable references must never rebind to a later Card that happens to reuse an address. [Workspace rewrite protection](E:/zet/src/workspace.rs:88) · [Link index maintenance](E:/zet/src/link_index.rs:284)

### Lifecycle and authority

| Operation | Proposed transaction |
|---|---|
| Edit a mounted Card’s body | Resolve the Card, show its home and owner, then use the normal home edit, gate, and Lock checks. All placements show the result. |
| Edit “why here” | Write the host placement only. Check host Workspace, write scope, and host Lock. No source Card rewrite or generated Reverse link. |
| Move a home Card | Apply existing home-boundary, rewrite, Lock, and pin rules. Same-Workspace Topic-to-Topic moves remain possible where currently permitted. Literature Cards retain the existing closed-tree rule. Mounts follow identity. |
| Move a placement | Move within the host tree by placement ID. Preview subtree, resulting coordinates, and changed rationale endpoints. A transfer to another host is an explicit place-and-detach operation checked against both hosts. |
| Detach a mount | A leaf can be detached directly. For a nonleaf, require an explicit subtree removal or a specified reattachment for surviving children. A plan that would detach a native Card from its sole home is refused until its disposition is explicit. |
| Delete a Card or home subtree | Enumerate ordinary link effects and all affected placements, descendants, and rationales. Default to refusal while mounts remain. An explicit combined plan may detach mounts and delete the Card only when every affected host and the home permit it. |
| Empty or delete a Reading tree | Remove its placements. Enumerate its native Cards as owned content subject to deletion and check their outside placements. Mounted targets survive. `keep-root` retains the native root and its identity. |
| Delete or empty a Literature tree | Preserve the current root and `keep-root` distinction. Include every mounted home descendant in the impact check. A mount of the retained root remains valid. |

The existing move planner produces address mappings, and existing operations use an edit lock, preview, and transactional application. Extend that discipline with an **expected revision and plan digest** for destructive placement changes. A confirmation made before another editor inserts a branch must not authorize removal of that new branch. A successful retry must not create a second placement or advance the revision twice. The uniqueness constraint plus a request key can distinguish an exact replay from a fresh conflicting request. [Move and tree operations](E:/zet/src/ops.rs:642) · [Existing home boundaries](E:/zet/src/main.rs:4850)

### Locks, gates, derived data, and workflows

- **Locks and pins.** Mounting a human or Locked Card in an unlocked agent Reading tree is a read of the target and a write to the host. The reverse operation requires human authority over the host. Locking a Reading tree freezes its topology, its rationales, and its native authored Cards. Mounted targets receive the existing external-target identity/address promise; their source text can subsequently change unless its own home is Locked. The Lock preview should name that promise explicitly. An owner who needs exactly the text they read at a given time can retain a dated portable export. Keep the existing owner-controlled Lock lifecycle, SQL trigger protection, integrity latches, and historical seals. A Reading Lock needs new evidence and triggers because today a Lock derives its members by home-address prefix. [Lock planning](E:/zet/src/tree_lock.rs:1663) · [SQL protection](E:/zet/src/tree_lock.rs:170)
- **Write gate and `zt-check`.** A Card body goes through the gate once, in its home context. A placement gets structural, access, scope, and rationale checks in the same transaction. The Literature-scoped agent connection must explicitly reject Reading-tree mutations and apply its outbound-target restrictions to new address forms. An allow-listed writer cannot turn permission to place a Card into permission to edit it. A batch that combines an allowed placement with a forbidden Card edit rolls back. [Write gate](E:/zet/src/gate.rs:701) · [MCP write scope](E:/zet/src/scope.rs:107)
- **Model, Math, Term, and Cited computation.** Introduce distinct operations such as `home_parent(card_id)` and `placement_parent(placement_id)`. Require structural reports to choose the former. Model membership also retains its explicit shared parts and `**引入**` import rules. A proof step mounted below another theorem does not become part of that theorem. A Term head mounted under an Event remains the same head; a Cited Card retains its home list row. Test report equivalence before and after mounting all four types. [Model head and imports](E:/zet/src/model.rs:778) · [Term placement](E:/zet/src/term.rs:184) · [Cited roles](E:/zet/src/cited.rs:110)
- **Links, Reverse links, and search.** Keep Card bodies authoritative for reasoning Links and Term uses. Placements are authored records in their own right. Show them under **Also in trees**. Generating an ordinary Reverse link for each mount would turn curatorial reuse into false evidence of intellectual influence. `zt reindex` should continue to rebuild the Link index from text. A separate tree integrity check should verify placement structure and rebuild disposable coordinate caches. Search should return a Card once with its available reading contexts; an exact Reading coordinate can return the placement. Bump the archive revision on committed placement changes so GUI and MCP readers agree. [Link rebuild and audit](E:/zet/src/link_index.rs:543) · [GUI snapshot](E:/zet/src/gui.rs:1349)
- **Hubs, Spreads, and existing jobs.** Keep Hub levels and range checks over their existing home domain. A Hub can point readers to the native root of a Reading tree through an ordinary Card Link without acquiring its members. A Spread still holds Cards on a Table; positional coordinates and layout are different data. `zt-idea-grow` should continue to produce standalone Topic claims and maintain Hubs and the Register. A mount earns no credit toward grown Cards, hub coverage, or deduplication. `zt-post-digest` should still fold, structure, import, illustrate, and organize the source book under its existing contract. A later optional job could propose cross-book readings after those passes. Round 1 gives no reason to change the current jobs or their sizing. [Idea-grow workflow](E:/zet/docs/wiki/idea-grow-workflow.md) · [Post-digest sequence](E:/zet/skills/zt-post-digest/SKILL.md) · [Digest job map](E:/zet/docs/agents/digest-job-map.md)

### Migration, dump, and portable export

**The zero-data-loss criterion has to be demonstrated on an archive copy.** A successful `ALTER TABLE` proves very little in a system whose behavior depends on derived text, Lock fingerprints, and parent arithmetic.

1. **Inventory and rehearse.** Capture a consistent SQLite backup plus the matching artifacts. Record schema/protection generation, SQLite integrity, archive identity, revision, existing Lock faults, authored text and BibTeX hashes, row identities, addresses, and a baseline of derived reports. Include an old archive, an actively Locked tree with external pins, an ended Lock, an existing Broken link, and a human Card linking to an agent Card.
2. **Install the additive identity and Reading tables behind a writer-version boundary.** Backfill exactly one ID per extant Card. Preserve `row_id`, addresses, authored bytes, Workspace, shelf metadata, Topic allocation, Lock history, and active pins. Preserve and verify legacy seals under their original algorithm. Never treat migration as permission to reseal existing Locks. First demonstrate that an older executable cannot mutate the upgraded archive through *any* local writer path; the CLI and GUI can open SQLite directly. If this compatibility barrier cannot be demonstrated, keep the converted archive separate from the original.
3. **Prove equivalence with zero Reading trees.** Compare home trees and reading order, all authored hashes, ordinary and Term-link occurrences including duplicates and Broken targets, Reverse sections, Model/Math/Term/Cited/Hub reports, Spread membership, Lock integrity, and denied-write behavior. Then create mounts and repeat the comparison for the same home data. Crash or abort at each migration boundary and demonstrate recovery from the retained backup.
4. **Version the interchange formats.** `zt dp` should capture one body per Card plus identities, tree definitions, native homes, placements, rationales, pins, and Lock evidence from the same revision. Verify by reconstruction on a scratch archive, including identical stable bookmark resolution. `zt export ~7` should capture one route’s placement order and one rendered copy of each included Card body. Its default closure is the native and explicitly mounted Cards. A mounted book root contributes its root Card; embedding its chapters requires an explicit expansion. Show the inclusion list and cross-Workspace count before packaging. An offline reference outside the package should say **outside this export**, distinct from **broken in the archive**. Use package-local IDs and preserve the current exclusion of archive paths, internal row IDs, and Lock facts. [Dump manifest](E:/zet/src/dump.rs:143) · [Portable export contract](E:/zet/src/export.rs:1)

I would ship **reading and placing existing Cards first**, then native authoring and Reading Locks once their migration and protection tests pass. A release that can write Reading trees must already preserve them in backups and dumps. The verification target is a reversible change of representation with measured equivalence, not a claim that inspecting the source has established migration safety.

## 5. GUI, Session, and MCP

**The GUI should keep two facts visible at the point of reading.** For example, the breadcrumb might read **Ownership theories › Hart**, and the Reader header might read **Home: Hart1995 · Literature · human**. Under the breadcrumb, display the placement rationale. Under the title, display the source Card. An editor should be able to tell which Workspace would receive an edit before opening an editor.

- **Cabinet.** Add Reading trees under each Workspace, searchable by title, question, and optional purpose. Distinguish counts of owned Cards from counts of placements.
- **Folgezettel map.** Lay out the active tree’s placements. Show the current Direct continuation and visible Side branches. The map of a 15-placement route should query approximately that route and visible summaries, even if one mount is the root of a 5,000-Card book.
- **Reader and return.** Offer **Continue**, **Open home tree**, and **Also in trees**. The switcher lists tree title, coordinate, and the immediate rationale. A normal Card Link can open its target within the active route if that target is placed there, with an explicit indication of context. Otherwise it opens home and pushes the original position onto the return stack. Repeated visits across mutually mounted roots remain ordinary navigation history; no renderer recursively expands them.
- **Dependency lens.** At a theorem, show immediate prerequisites and users first. Selecting an edge reveals its sentence or proof step. Keep the route breadcrumb visible when opening the lens. Back restores the exact teaching station.

**The terminal Pointer should identify the Card and its occurrence.** For a Reading tree, persist `(card_id, tree_id, placement_id)` and derive the visible coordinate. Legacy home positions can initially resolve through the home adapter. `go '~7/2'` enters a current slot, `home` opens the source, `places` lists other readings, and `back` restores the prior occurrence. On detach, an open Session should report **placement removed** and offer the surviving Card’s home. It must never land on the new occupant of slot two without telling the reader. For mutations use explicit verbs such as `unplace` and `edit-reason`; preserve `del` as an operation on owned Cards with the usual preview. [Session Pointer handling](E:/zet/src/session.rs:362) · [GUI tree and Card routes](E:/zet/src/gui.rs:1293)

**MCP should expose typed targets and bounded reads.** Keep `get_card` and legacy home-tree tools compatible. Add `get_reading_tree`, `list_placements(card_id)`, `place_card`, `create_reading_card`, `edit_placement`, `move_placement`, and `unplace`. Return `card_id`, `home_address`, `home_workspace`, `host_workspace`, `placement_id`, displayed coordinate, and revision where applicable. Mutation calls should distinguish `card_id` from `placement_id` at schema level. A host edit may be allowed while a content edit is refused on the very same screen. A single `writable: true` would be incorrect. Return `can_edit_card`, `can_edit_placement`, or the corresponding typed refusal. [MCP dispatch and error handling](E:/zet/src/mcp.rs:288) · [Current bounded tree reads](E:/zet/src/read.rs:702)

## 6. What I want Gemini to challenge in Round 2

| Decision | My Round 1 position | Counterexample or test that could change it |
|---|---|---|
| Repeated appearances | One coordinate per Card per tree. Arbitrarily many recall Links. | Construct a lesson where a second coordinate improves orientation and where “recall this definition here” fails. What should `next`, Back, progress, and search mean on both visits? |
| Canonical home | Every Card has one. Existing specialized structures use its home context. | Demonstrate a useful Card that truly needs two structural homes, and specify which home validates it when the two impose different rules. |
| Reading-native addresses | Give them stable local numbers such as `~7/c4`, separate from visible positions. | Test whether readers can reliably copy the intended Card or place link. If the `c` spelling causes mistakes, change the spelling before changing the identity model. |
| Locks on readings | Freeze editorial order and rationale. Pin mounted identity/address. Source content can evolve. | For a historian reopening a reading a year later, is a dated export adequate, or must the live Reader flag “source revised since this reading was locked”? This is the strongest case for storing a target fingerprint at Lock time. |
| Deleting a widely mounted Card | Require explicit disposition of its placements. A protected host can block deletion. | Suppose an obsolete agent source is mounted in 80 readings, including a human one. Would a visible tombstone preserve both editors’ authority better? If so, define how a missing station reads and which descendants survive. |
| Reading tree versus Hub | A Hub selects entrances. A Reading tree authors continuations. Both may serve the same question. | Give a real Hub whose groups encode an indispensable reading order today. Should its editor be able to derive a route once, and which object would own later edits? |
| Strength of editorial claims | Keep a short local reason on an edge. Put an independently contestable argument on a citable Card. | Take a disputed historical transition and identify exactly when a reason needs its own source, author, revision history, or inbound Links. That determines whether placement annotations can remain lightweight. |

**My acceptance exercise for the design review** would give a reader one example of each kind and ask them to do five things unaided: resume the comparison after inspecting Hart’s book, identify both premises of the theorem, distinguish a dated observation from a causal claim, say who owns the text currently on screen, and remove a paper from a reading without deleting its source. A design that fails one of those tasks needs revision even if every database constraint passes.

---

## Round 2 — Gemini's Critique & Pushback to Astra

# Round 2 Critique & Convergence Discussion (from Gemini)

Thank you for a remarkably sharp Round 1 memo. Your grounding in ZET's actual internals (`cards.row_id`, `tree_lock.rs`, `math.rs::theory_report`, `ops::zone`, `place.rs`, `read.rs`, `export.rs`) and your core invariant — **every Card has one canonical Home plus 0..N Placements across additional trees, each placement carrying a local edge rationale** — is rock-solid. I also strongly agree with your "Hart return-stack" acceptance test and your rule that **changing a placement's parent invalidates/requires review of the incoming rationale**.

Now let's push hard on the 7 deepest architectural tensions and your Section 6 challenges so our final Markdown proposal is completely consistent, cognitively ergonomic, and free of hidden traps.

---

## 1. Three Zones (`Literature` / `Topic` / `Reading`) vs. Two Zones (`Literature` / Unified Non-Literature Tree)?

This is our highest-leverage ontological decision. Let's test both sides honestly:

- **Case for your 3-Zone split (`Literature`, `Topic`, `Reading`):**
  - Preserves ZET's existing Topic-tree invariants untouched (`zt-idea-grow`, `hub.rs` H1–H13 missing-topic-hub checks, `zt-check` standalone claim rules I1–I7, `topic_exists` title uniqueness, legacy positional addresses `3/2|a`).
  - Avoids polluting the Luhmann "main box" (`Topic` zone) with pure Literature-Group comparison trees (e.g., 12 mounted Literature roots with 1 root card) that should not trigger "missing Topic Hub" or "low Topic claim density" audits.
  - Gives clean three-way mental separation: *Source provenance* (`Literature`) vs. *Atomic personal/agent arguments* (`Topic`) vs. *Cross-tree curated organization* (`Reading`).

- **My pushback / stress test against 3-Zone:**
  1. **The "Side-Evidence in a Topic Tree" scenario:** Suppose a user is developing a Topic tree `3/0` ("Fiscal Decentralization") with 15 Idea Cards (`3/1`, `3/2`, ...) and wants to mount an Event Card `HistoryA/2|a` right beside `3/2` on the Folgezettel map so they can inspect the evidence in place without leaving the Topic tree. Under a strict 3-Zone model, they *cannot* — they must create a separate Reading tree `~7` and mount both their Topic Cards and the Event Card!
  2. **The "Synthesis Tree Matures into Ideas" scenario:** Suppose a user starts a Reading tree `~7` for a History Thread (Example 3), writes 10 native synthesis Cards (`~7/...`) making original claims, and later wants those claims indexed in the Register, covered by Hubs, or continued by `zt-idea-grow`. Are Reading-native Cards second-class Idea Cards, or first-class Idea Cards that participate in search/Register/Hubs? If they are first-class, what really separates a Reading tree with mostly native Cards from a Topic tree?
  3. **Premature-choice friction:** When opening a new line of inquiry that mixes personal claims, papers, and historical events, the user must guess upfront: *"Topic tree or Reading tree?"*

- **Question for you:**
  Should we:
  - **(A) Keep 3 Zones (`Literature`, `Topic`, `Reading`)**, make Reading-native Cards first-class searchable/linkable Idea Cards, and support explicit **promotion** (`move_home` of a Reading-native Card into a Topic tree while keeping its placement in the Reading tree)?
  - OR **(B) Unify Topic and Reading in the long-term data model** (one placement engine for non-Literature trees, with a tree role/shelf attribute like `topic` vs `reading` controlling Hub/grow policy), while shipping Reading trees as the first placement-backed tree kind in Phase 1?
  - Also, **naming check**: Does **"Reading tree"** fit Example 1 (a literature-group problem map) and Example 3 (a historical evidence thread), or does "Reading" sound too passive/textbook-like? How does **Reading tree** compare to **Thread tree**, **Trail tree**, **Folio tree**, **Map tree** (note: `Folgezettel map` already uses "map"), or **View tree**? Let's pick one crisp `CONTEXT.md`-grade name.

---

## 2. Address & Link Syntax: Pushing Back on `~7/c4` and `zt-place:<archive-id>/<placement-id>`

I **100% agree** with your core engineering principle: **reordering a Reading tree must NEVER rewrite Markdown `[[...]]` Links across the archive** (which would trigger `ensure_link_rewrites_allowed` human-Card refusals and Lock pin conflicts on every reorder), and **positional slots (`~7/2`) must NOT be accepted as authored Card Links** (because `~7/2` could silently point to a different paper tomorrow).

**However, I push back strongly on the spelling `~7/c4`:**
- Visually, `~7/2` (mutable coordinate, forbidden in `[[...]]`) and `~7/c4` (immutable native Card ID, required in `[[...]]`) look dangerously alike! Both use `~<tree>/<something>`. Worse, `~7/c0` vs `~7/0` will trip humans and LLMs constantly ("Wait, why is the root `~7/0` in `go` and `~7/c0` in `[[...]]`?").
- Look at a cleaner visual invariant: **Make `/` ALWAYS mean a Folgezettel tree coordinate path, and NEVER use `/` for a non-positional ID!**
  - Tree coordinates (mutable positions, used in `go`, Map, breadcrumbs, `place`/`move`): `~7/0`, `~7/1`, `~7/2`, `~7/2|a`.
  - Native Card canonical address (immutable, valid in `[[...]]`): **`~7#4`** (and root **`~7`** — just like a Literature root is bare `Hart1995` with no `/0`! Or `~7#0` for explicit root, `~7` as root canonical address).
  - Check conflicts: `#` is never used inside `[[...]]` in ZET (`src/link.rs` only allows locations, citation keys, and `label@location`). Visually, `[[~7#4]]` immediately shouts *"stable Card #4 belonging to tree ~7, NOT a path position"*, while `~7/2|a` immediately shouts *"position 2|a in tree ~7"*.
- Second, on **`zt-place:<archive-id>/<placement-id>`**:
  - Look at ZET's existing link schemes: `artifact:<sha256>` and `scholar:<file_id>` (neither carries `<archive-id>`).
  - Could a durable occurrence bookmark in Markdown simply be `[label](place:<placement_id>)` (or `zt-place:<placement_id>`), keeping `<archive-id>` only in external OS deep links if ever needed? Keeping Markdown clean matters enormously for human comprehension.

What do you think of `~7` (root) / `~7#4` (native Card canonical address) vs `~7/2|a` (mutable coordinate) + `[label](place:<placement_id>)`? Do you see any parser or shell edge cases (note `#` in bash/PowerShell comments — though in ZET shell commands `|` already needs quoting, wait: unquoted `#` at start of arg in PowerShell is a comment, while `~7#4` mid-token is NOT a comment in PowerShell or bash; still worth checking! What about `~7:4` or `~7.4` vs `~7#4`)?

---

## 3. Repeated Appearances (`UNIQUE(tree_id, card_id)`) & Example 2 (Math Theorem ↔ Definitions)

I **agree** with keeping `UNIQUE(tree_id, card_id)` (at most one coordinate per Card per tree). Here is why I think your provisional rule is actually the *right* permanent rule from first principles:
- Unrolling a DAG (Theorem 1 uses D1, Theorem 2 uses D1) by giving `D1` two coordinates in the same tree **destroys the single most important cognitive insight in mathematics**: seeing that multiple results converge on the *same* foundational definition/lemma (shared load). It also creates ambiguous search landing, ambiguous "where am I in this tree?", and duplicate progress state.

**However, to truly satisfy the user's Example 2 ("math lit, one thm can link to another defs, and reader can learn this relationship more clearly"), the Dual-Lens design must be explicit and first-class, not a minor bullet:**
1. **Cross-Book Local Dependency Lens (Computed DAG, not limited to one Theory Card!):**
   - Today `math::theory_report` only operates over the units explicitly listed in a single `Theory Card`'s `**结构**` section (`src/math.rs:834`). It does NOT give a general cross-book prerequisite graph for *any* Theorem/Model/Claim Card `T` (e.g., Theorem in Book B resting on Definition in Book A, resting on Axiom in Book A).
   - Proposal: Add a first-class **Dependency Lens** (computed on demand from `links` where `kind = 'rests-on'`, with optional toggle for other reasoning kinds and strictly excluding `kind = 'term'` unless requested) for *any* Card — especially Math Units and Model Cards:
     - Shows multi-hop prerequisites (bounded depth, e.g., depth 1–3, cycle-safe) and direct dependents ("Used by"), across **all Literature and Topic trees**.
     - **Anti-split-attention reading (Sweller):** Clicking any prerequisite node (e.g., `Def D1`) opens it in a **side-by-side companion pane** beside Theorem `T`, highlighting the exact authored sentence / proof step where `T` invokes `D1` (and distinguishing `rests-on` premise from `term` vocabulary gloss).
2. **Bridge between Computed DAG and Authored Tree ("Seed Study Tree"):**
   - One action on the Dependency Lens: **"Seed Reading tree from prerequisites"** — computes the topological dependency order of `T`'s closure and drafts a new editable Reading tree where the learner can insert native intuition/example Cards and edge rationales, while the live Dependency Lens remains one toggle away!
3. **Recall cue inside a Reading tree:**
   - Since `UNIQUE(tree_id, card_id)` prevents mounting `D1` twice, when `U` needs `D1` again later in the same Reading tree, `U`'s placement rationale or a native transition Card (or `U`'s own body) links `[[D1]]`, and clicking `[[D1]]` inside the Reading tree opens a **quick peek / side-card** (or jumps to `D1`'s coordinate in `~7` with Back returning to `U`), keeping working memory intact without duplicating nodes!

Does this Dual-Lens + Bridge + Quick-Peek fully close Example 2? Any flaw in computing cross-book `rests-on` DAGs directly from `links` (e.g. `src/link_index.rs` already indexes `(source, ord, target, kind)`!)?

---

## 4. Pushing Back on Locks & Source Drift (Your Challenge #4)

You asked whether a Locked Reading tree should store mounted targets' `authored_fp` at Lock time so the live Reader can flag *"source revised since this reading was locked."*

**Strong YES — and here is the exact distinction to keep Locks sane:**
- Locking Reading tree `~7` makes **two distinct promises**:
  1. **Hard Freeze (blocks writes via triggers + gate):** `~7`'s tree structure, placement rationales, and native Cards cannot change; mounted external Cards are **pinned** (their `card_id`, `row_id`, existence, and canonical address cannot be deleted or renamed).
  2. **Advisory Provenance Seal (recorded, never blocks source edits):** `lock_pins` for Reading-tree mounts ALSO records `observed_authored_fp` of each mounted target at Lock time.
- If the external source Card is edited later (when its own home tree is unlocked):
  - The edit **succeeds** (locking a Reading tree never freezes someone else's unlocked Literature or Topic tree!).
  - The Reading Lock's integrity stays **intact** (NOT a tampering fault!).
  - Instead, the Reading Lock report and the GUI Reader display a calm, factual **"Source revised since lock"** indicator on that placement (with a one-click "Review & Reseal" action for the Owner).
- This gives historians and researchers **both** non-blocking autonomy AND full epistemic visibility when an underlying source note evolves! Do you agree, or do you see any trap with `tree_lock.rs` generation/trigger complexity?

---

## 5. Pushing Back on Deleting Widely Mounted Cards & Cross-Workspace Authority (Your Challenge #5)

Look closely at the authority trap you identified:
- Agent creates Card `A` (unlocked agent tree).
- Human mounts `A` into a **human** Reading tree `~1` (unlocked), and hangs human synthesis children `~1/2|a` and direct continuation `~1/3` under `A`.
- Later, Agent (or `zt del --keep-root` re-digesting an agent book!) tries to delete `A`.

If we ONLY allow "refuse while mounted, unless caller has write access to every host tree":
- **An unlocked human mount silently acts like an invisible permanent Lock on agent Cards!** The agent can never re-digest or delete its own unlocked book without the human manually detaching mounts first.
- Compare with how ZET ALREADY handles `[[links]]`:
  - A **Locked** tree pins its targets -> **HARD REFUSAL (`pinned-delete`)**.
  - An **unlocked** human Card linking `[[A]]` does NOT block deleting `A` -> instead, `A` is deleted and the human Card gets a **visible Broken Link** (`lsbk`), preserving human authored text untouched!

**Proposed resolution (Symmetric with Broken Links — "Missing Mount / Tombstone"):**
- **Case 1 (Locked host tree):** Always **HARD REFUSAL (`pinned-delete`)** for both Owner and Agent.
- **Case 2 (Unlocked host tree, Owner deleting):** Preview enumerates all mounts across all trees. Owner explicitly chooses one of:
  - `detach` (for leaf mounts or with explicit child disposition), OR
  - `tombstone` (leave a **missing-Card station** in the host tree).
- **Case 3 (Unlocked human host tree, Agent deleting an agent Card — or Owner choosing `tombstone`):**
  - Instead of either deadlocking the agent OR silently destroying the human's Reading tree children/rationales, the placement becomes a **Missing Mount (Tombstone)**:
    - `placement.card_id` becomes `NULL`, while `missing_address` (last canonical address) and `missing_title` (last title) are snapshotted on the placement row.
    - The placement's **position, incoming rationale, and all descendants (`~1/2|a`, `~1/3`) survive intact!**
    - The Reader renders a clear muted stub card: *"Mounted Card deleted (`BookA/4|b` — <old title>). Structure and children preserved. [Rebind to another Card] [Detach / Reattach children]"*.
    - `zt lsbk` / tree health audit lists broken mounts right beside Broken links!
- **Tradeoff check:** Does allowing nullable `card_id` (only when `tombstone_address IS NOT NULL`) weaken schema integrity, OR is a `CHECK((card_id IS NOT NULL AND missing_address IS NULL) OR (card_id IS NULL AND missing_address IS NOT NULL))` clean, explicit, and far safer than cross-workspace deadlock? Note: a Reading tree's **root** can never be tombstoned (deleting a Reading tree deletes or empties the tree).

---

## 6. Two Subtle Edge Cases to Nail Down

1. **Placement Rationale vs. Native Claim Card (Your Challenge #7):**
   - Let's formalize the **"Gloss vs. Claim" principle**:
     - A **placement rationale** is strictly a **positional reading gloss** (plain text, bounded e.g. ≤ 240 chars, optional Relation kind, no `[[links]]`, not independently citable, no source line). It answers *"why read this Card at this position?"*
     - Any **contestable claim, causal inference, synthesis, or multi-source reconciliation** MUST be a **native Card** (full Markdown, `来源`, typed `[[links]]`, citable, refutable via `反驳`).
     - UI provides a one-click **"Promote gloss to Card"** action that inserts a native Card carrying that text as a draft claim before/beside the placement!
2. **Mounting Another Reading Tree's Root & Selective Sub-Mounting:**
   - Confirm: Can Reading tree `~8` ("Theory of the Firm Overview") mount Reading tree `~7`'s root (`~7`) as a single node — just like mounting `Hart1995`? And can `~7` mount both `Hart1995` (at `~7/2`) AND a specific key Model Card `Hart1995/4|a` (at `~7/2|a`)?
   - Under `card_id`-based mounts with no recursive expansion, both work automatically! Should we forbid mounting a Reading tree's own root inside itself (trivial via `UNIQUE(tree_id, card_id)`), and allow cross-Reading-tree root mounts purely as navigation portals (never recursively expanded)?
3. **Hubs & Spreads Integration (Your Challenge #6):**
   - Keep **Hubs** (flat grouped entry directories), **Spreads** (2D working Table canvases), and **Reading trees** (ordered Folgezettel spines + branches) strictly orthogonal — NEVER two-way synced.
   - Support one-time directional **seeding**:
     - `Spread → File as Reading tree` (alongside existing `File as hub card`)
     - `Hub → Seed Reading tree`
     - `Dependency Lens → Seed Reading tree`
   - Each creates an independent Reading tree owned by its editor from that moment on.

---

## What I'd Like from You in Round 2

Please critique every pushback above ruthlessly:
1. Where do you agree, and where do you still disagree (especially on **3-Zone vs 2-Zone**, **Tree Name**, **`~7#4` vs `~7/c4`**, **Tombstones vs Refusal**, and **Lock `observed_authored_fp`**)?
2. Check any concrete codebase constraints in `E:\zet` (`src/link.rs`, `src/tree_lock.rs`, `src/link_index.rs`, `src/main.rs`) that might break or complicate any of our Round 2 refinements (e.g., `#` in `LinkBody::parse`, `links` table `kind` column for the Dependency Lens, SQLite triggers for tombstones).
3. Once we settle these points, confirm the final unified architectural decisions so I can compose the complete, authoritative Markdown design proposal reflecting our converged design!

---

## Round 2 — Astra's Final Decision Memo (`gpt-6.2-astra`, high)

# Round 2 decision memo — Multi-tree coordinates in ZET

I would adopt **B for the architecture**: Topic and Reading use a common placement model, while Literature, Topic, and Reading remain three named roles in the Cabinet. I would also revise two Round 1 decisions. A Topic should be able to display mounted evidence beside a home Card, and an unlocked foreign mount should survive deletion of its target as a **Missing mount**. The human who arranged the evidence and the agent who maintains the source book can then both exercise their existing authority.

I accept **`~7`**, **`~7#4`**, and **`~7/2|a`**. I also accept capturing a source fingerprint when a reading is locked, with one important change to the proposed action: **Review source** should append an observation. **Reseal** already means something materially different in ZET.

## 1. Three recognizable roles, one placement model

An author opening a new inquiry should decide what they want to do now. They should not have to predict whether ten original claims will eventually grow out of a comparison of three books.

| Tree role | What the reader can rely on | What authors can do |
|---|---|---|
| **Literature** | The filing reflects one source. Structural ancestry has meaning for its Models, Terms, and Cited Cards. | Organize the work under its existing rules. Place any of its Cards in another tree. |
| **Topic** | The numbered home spine follows the development of the Topic’s own Cards. | Continue a claim, branch from it, and mount outside evidence beside it. Topic and Hub audits count the relevant home Cards. |
| **Reading** | One editor has chosen a sequence and branches for investigating a question. | Interleave mounts and native Cards on the spine or Side branches. Publish original synthesis where it arises. |

**The decisive constraint is that a Topic mount must never consume a legacy home slot.** Suppose `3/2|a` already holds an Idea. Mounting an Event beside `3/2` must leave `3/2|a`, its descendants, every `[[3/2|a]]`, and every Term or Model calculation intact. Even an *empty* `3/2|b` cannot safely be borrowed. A later home insert would have to displace the evidence or rewrite its meaning. ZET currently derives parents and successor coordinates from the Location string itself. [Location and parent computation](E:/zet/src/main.rs:2705) · [Placement planner](E:/zet/src/place.rs:1)

I would give Topic evidence its own ordered set of side placements anchored by **home Card identity**. For example, `3/2+1` means “first mounted item beside `3/2`.” It is a navigation coordinate. It cannot be written as `[[3/2+1]]`. On the map, the Event appears beside the claim with its source and local reason. Direct continuation still goes to `3/3`. The anchor’s home rename changes the displayed coordinate while preserving the placement bookmark. A home operation that deletes the anchor or moves it to another Topic must preview the affected attachments and their disposition. The initial Topic interface can keep these attachments as leaves. A Reading tree already supports the more demanding operation of weaving foreign material through a spine and giving it children.

This resolves the fiscal-decentralization example without requiring a second tree. It also makes the rollout honest: Reading trees can ship first, but **the Topic-side evidence scenario is complete only when Topic attachments ship**. Sharing a storage engine does not require making every operation legal in every role.

**Reading-native claims are first-class Cards.** A substantive native Idea can appear in search, receive ordinary Links, be selected as a Hub member, and serve as an explicitly chosen Register destination. A transition such as “Next compare the municipal accounts” has little claim to a Register entry merely because its Card type is `idea`. Eligibility should depend on what the entry explains. Hubs retain their home-range coverage rules, and `zt-idea-grow` continues to file new Topic claims under its existing contract. Explicit Register selection of a substantive Reading claim would extend today’s documented target policy and should be specified as such. [Register contract](E:/zet/CONTEXT.md:428) · [Hub coverage and reach](E:/zet/src/hub.rs:1773)

**Promotion changes home while preserving identity.** A same-Workspace `move_home` can give an eligible Reading-native claim a Topic home. Its Reading placement, incoming gloss, children, and bookmarks survive. Only the selected Card changes home. Its Reading children remain where the reading’s editor put them. The destination, gate, Lock, pin, and write-scope checks run in one transaction. A Reading root cannot be promoted this way. If the destination Topic already contains a mount of the claim, the preview must resolve that occurrence explicitly so that the Card still appears once in the Topic.

**Name: Reading tree.** I would put this definition in `CONTEXT.md`:

> A **Reading tree** is an authored Folgezettel tree that leads a reader through a question. Its spine and branches can use Cards from other trees and Cards written for the reading. Each Card has at most one position in the tree.

“Thread” fits the history example, but suggests a single line and competes with ZET’s existing lines of thought. “Trail” suggests where someone has already gone. “Folio” conveys a collection without its order. “Map” already names the Folgezettel display. “View” suggests a disposable projection, which would be especially misleading once a tree owns synthesis Cards and can be locked. A problem comparison is a *reading of the literature* in the active, interpretive sense. **Reading tree** covers it.

## 2. Address grammar — accept `#`, and settle what survives promotion

| Spelling | Contract |
|---|---|
| `[[~7]]` | Cite the root Card of Reading tree 7. Use this single canonical spelling for the root. |
| `[[~7#4]]` | Cite the Card originally issued number 4 in Reading tree 7. Its handle survives reordering **and promotion**. |
| `~7/0`, `~7/2\|a` | Navigate to a current position. `go '~7/0'` and `go '~7'` initially show the same root through positional and Card lookup, respectively. |
| `[return to this station](place:<uuid>)` | Reopen one placement in the current archive. Reordering follows the occurrence. Detachment retires it. A Missing mount opens its surviving station. |

I prefer `#` to `:` or `.`. It makes the local number visible, keeps `/` for tree traversal in the new grammar, and avoids overloading a URI scheme separator. Existing links such as `[[3/2]]` retain their historical meaning. The ban on citing a mutable slot applies to the **new placement coordinates**.

I checked the implementation and the shell behavior. The shared `[[…]]` scanner splits ordinary and Term forms on `@`, preserves byte ranges, and passes `~7#4` through lexically. **The current validator rejects it.** Strict parsing, tolerant lifecycle classification, indexed validation, Card lookup, and read-side joins all need the new Card-address resolver. Merely accepting `#` in `LinkBody::parse` would let one path write a Link that another reports as broken. Term links should retain their head-target rule. I also passed unquoted `~7#4` through the current PowerShell and Git Bash; both returned the literal token. I would still quote coordinates in shell examples because `|` is a pipe and a leading `~` can have shell-specific expansion. In GUI URLs, encode `#` as part of the address. [Shared scanner](E:/zet/tools/zt-check/src/text.rs:884) · [Strict parse](E:/zet/src/link.rs:25) · [Lifecycle classification](E:/zet/src/link.rs:441) · [Indexed lookup](E:/zet/src/link_index.rs:463) · [GUI routing](E:/zet/src/gui/app.js:1025)

Promotion exposes a deeper issue than punctuation. **Once published, `~7#4` is a permanent citation handle.** After promotion, the Reader might show “Home `3/6` · cite `~7#4`.” Both resolve to the same Card. The issuance number and tree number are never reused, including after the original Reading tree is deleted. Name resolution and Reverse links must group by resolved Card identity while preserving the authored spelling and occurrence. A move of `3/6` still applies legacy protections to literal `[[3/6]]` references. `[[~7#4]]` needs no rewrite. This is the cost of promising both promotion and stable citations. I would make it explicit in the schema as an issuance record independent of current home.

**`place:<uuid>` is sufficient within Markdown.** Use a globally generated placement UUID, resolve it only against the open archive, and report an unavailable destination if it is absent. An external launcher can add archive identity when it needs to choose which archive to open. The GUI and portable renderer should dispatch `place:` deliberately, as they already do for ZET’s other internal destinations. A place bookmark is navigation. It supplies no reasoning edge, Term use, or Lock pin. A portable package can remap included bookmarks to package-local IDs and mark excluded destinations as outside the export. [Internal Markdown destination handling](E:/zet/src/gui.rs:2481)

## 3. The Dependency Lens is a primary feature

**Yes to one coordinate per Card per tree. Yes to the lens, companion pane, and editable seed.** The prerequisite example justifies all three. I would use **dependency graph** as the general term. Authored `rests-on` relationships can contain cycles, unresolved targets, and relationships the math checker considers faulty. Calling every result a DAG would conceal exactly what a reader sometimes needs to discover.

The Link index is sufficient to **find candidate occurrences**. It stores `(source, ord, target, kind)`, indexes incoming targets, records Term uses as `term`, and stores an unqualified ordinary Link with a null kind. It stores neither the full sentence nor a persistent byte offset. A Card-level request can query outgoing `rests-on` rows or incoming users directly. Highlighting must reparse the source’s authored body at the same archive revision and resolve `ord` to the exact occurrence. Use the actual sentence and proof-step Card in the companion pane. The existing short display reason may be clipped or normalized. [Index construction](E:/zet/src/link_index.rs:98) · [Index schema](E:/zet/src/link_index.rs:159) · [Occurrence ranges](E:/zet/tools/zt-check/src/text.rs:755) · [Relation sentence rendering](E:/zet/src/model.rs:663)

The lens needs **two explicit scopes**:

1. **This Card.** Show the relationships authored in this exact Card and the Cards that use it. Available for a theorem, a historical claim, a synthesis Card, or any other Card in either Workspace. Other reasoning kinds and vocabulary can be revealed separately. Neither changes prerequisite closure.
2. **This unit or model.** Include evidence from the statement, premises, and relevant member or proof-step Cards. Show the actual carrier beside every aggregated edge. Reuse existing membership and import rules. If a step reaches inside another unit, show the real target and existing *reach-in* diagnostic. If a shared part serves multiple models, retain the chosen model context. Assigning every use to the first head found would misattribute some evidence.

Your correction to Round 1 is fair. `theory_report` starts with the units listed by one Theory Card. Other code already computes incoming citers and unit premises, including collapsing some part targets to their units. Those are building blocks for a cross-book lens. Neither a single Theory report nor `SELECT target FROM links WHERE source = T` answers “Which step in this theorem uses the definition in Book A?” [Theory membership and order](E:/zet/src/math.rs:836) · [Existing citer and premise computations](E:/zet/src/math.rs:359) · [Model reach-in diagnostics](E:/zet/src/model.rs:2089)

**Reader behavior.** Keep theorem `T` visible as the anchor. Selecting `D1` displays its definition alongside the exact sentence in `T` or its proof that invokes it. “Used by” can then reveal `U` without duplicating `D1`. When `U` occurs later in a Reading tree, a recall Link in its body or a native transition Card opens the same companion pane. **Go to its place** is a separate action with Back to `U`. The placement gloss stays plain text. This resolves the conflict between the proposed recall link in a gloss and the proposed no-Links gloss grammar.

**Seed behavior.** Start at a named revision with explicit depth and node limits. Distinguish outgoing prerequisites from incoming users. Collapse repeated target Cards while retaining every evidence occurrence. Detect cycles, distinguish nodes in a cycle from downstream blocked nodes, and expose broken targets and truncated frontiers. Propose prerequisites before users only for the resolved acyclic selection. If the selection contains a cycle, offer the editor a group to order manually. If bounded, label it a partial reading. Two independent prerequisites made adjacent by a tie-break acquire no invented relationship or gloss. Save produces an independently editable tree. A later source edit can change the lens; it cannot silently rewrite that tree.

## 4. Lock the reading and record what its editor saw

I agree with recording `observed_authored_fp` at lock time. I would specify three distinct records and actions.

| Record | What it establishes | What happens later |
|---|---|---|
| **Hard Lock evidence** | Frozen native content, placements, parent/order, glosses, and relation labels. Each live mounted target receives the existing external-target identity/address protection. | Illegal changes are refused. A violated hard promise is an integrity fault. |
| **Source observation** | Target identity, placement, fingerprint algorithm/version, observed fingerprint, and archive revision at lock time. | An otherwise permitted edit to an external source succeeds. A differing current fingerprint is advisory. |
| **Review acknowledgement** | The owner inspected a particular subsequent fingerprint at a particular revision. | Append the acknowledgement while preserving both the original baseline and the Lock seal. A later differing fingerprint can require review again. |

I would label the indicator **“Source differs from lock-time version.”** A fingerprint compares two states. An edit followed by an exact reversion yields the original fingerprint; “has been revised since lock” would promise an edit history the comparison cannot supply. A changed generated Reverse section should produce no indicator. The current authored fingerprint already excludes generated Reverse text. It also does not take every displayed field as an input, including `hub_level`, so the specification should name exactly what the observation covers. A claim about the mounted Card’s authored content does not extend to all descendants of a mounted book or all documents it cites. [Authored fingerprint inputs](E:/zet/src/card.rs:96)

Use **Review source** or **Mark reviewed**. The existing `reseal` event has a reserved migration meaning, and the code explicitly tests that no public reseal command exists. An acknowledgement is an append-only owner action with an expected observed fingerprint, so a simultaneous edit cannot be acknowledged by accident. Historical text or a line diff requires a retained version or snapshot. A hash alone cannot reconstruct the old passage. [Reseal contract](E:/zet/src/tree_lock.rs:6488)

There is a schema consequence. Current `lock_pins` identifies its origin by `source_row`, which denotes a **Card**. A placement has no source Card row of its own. Add versioned placement-pin/observation records keyed by Lock and placement, and extend SQL delete/move protection to cover the union of active body pins and placement pins. Preserve existing generations and ended-Lock evidence. For a reading that already contains a Missing mount, allow the owner to lock it only with the missing station shown in the preview. Freeze that station as observed; it has no live target to pin. Apply the same placement protection to Topic attachments when that capability ships. [Existing pin schema](E:/zet/src/tree_lock.rs:56) · [SQL pin guards](E:/zet/src/tree_lock.rs:170)

## 5. Missing mounts are the correct deletion behavior

Your agent re-digest example changes my Round 1 conclusion. **An unlocked human arrangement should have no veto over deletion of an otherwise deletable agent source. The agent should have no authority to rearrange the human’s reading.** Both conditions can hold in the same transaction.

I agree with the tombstone behavior and would implement it with a **persistent Card identity record**, rather than nulling `placement.card_id`. This is the most important data-model revision from Round 1.

| Record | On deleting a mounted source Card |
|---|---|
| Live `cards` row | Deleted through the normal home operation and Lock checks. |
| Card identity record | Becomes retired. Retains immutable Card ID, last canonical address, last title, and retirement revision. The live-row reference is cleared. |
| Foreign placement | Keeps the same non-null Card ID, placement ID, parent, order, rationale, and children. |
| Reader | Resolves that ID to a **Missing mount** and displays the last title and address. A different Card later issued the same legacy Location cannot fill the station. |

Your nullable-FK-plus-`CHECK` alternative is workable **if it also retains the retired Card ID**. Last address and title alone cannot distinguish two successive Cards at `BookA/4|b`. A persistent identity record gives a stronger invariant: `UNIQUE(tree_id, card_id)` still works for missing stations, foreign keys remain meaningful, and deleting one source mounted 80 times requires no authored update to 80 hosts. In particular, the agent does not issue an update against even one human placement.

The deletion plan should apply these rules:

- **Any active applicable Lock or pin** blocks the operation, for Owner and agent alike. Check the full plan, including collateral home-address compaction. Missing-mount behavior grants no exemption from an existing human-body rewrite refusal.
- **Unlocked foreign placements** default to Missing mounts. A writer authorized for a host may explicitly choose detach with a child-disposition preview. The ordinary agent re-digest retires its own deleted targets and preserves human stations. The `--keep-root` target stays live while removed descendants can become missing.
- **A native home occurrence** follows its owning tree’s home deletion rules. A tree’s own root can never become a missing root. If another tree mounted that root, the *external* station can become missing when the source tree is deleted.
- **Repair is a host edit.** Rebinding to a different Card requires checking uniqueness and reviewing both the incoming gloss and every immediate child gloss whose predecessor’s identity changes. My preference is to retire the old placement bookmark and issue a new one for the replacement. This keeps an old citation to “Hart at this station” from silently opening a different work. Before repair, the old bookmark continues to show the missing Hart station.
- **Audit and export** should report broken textual Links and Missing mounts under distinct types. Preserve the existing default machine-readable `lsbk` contract or version its expansion. An offline Card omitted from a package is a third state, **outside export**. Dumps must preserve retired IDs referenced by placements and historical evidence.

Today deletion drops source index rows, deletes Cards, updates Spreads, and applies compaction in one transaction. Retirement belongs in that transaction, before the old address or title can be lost. A direct SQL delete must either perform a valid retirement or fail closed. The release test should cover an interrupted deletion and confirm that the archive can never expose a retired target alongside a still-live Card or a surviving placement with an unknown identity. [Delete planning](E:/zet/src/main.rs:4336) · [Transactional deletion and compaction](E:/zet/src/main.rs:4496)

## 6. Glosses, mounted roots, and seeding

**Gloss versus claim.** Adopt a single-line positional gloss, bounded to 240 Unicode scalar values, with an optional controlled relation kind and no Markdown Links. “Read this next to compare their dates” fits. “The order caused the later practice” merits a native Card with the evidence, uncertainty, and source Links available for inspection. The gate can enforce length and syntax. Whether a sentence contains a contestable inference is an editorial judgment; a keyword classifier should not pretend to settle it. **Develop as Card** opens a draft prefilled with the gloss and the two endpoint Cards. Saving runs the Card gate and previews any changes of predecessor. To preserve the reading while developing the idea, offer a Side branch as the default.

**Root and selective mounts.** Permit `~8` to mount `~7`. Permit `~7` to mount both `Hart1995` and `Hart1995/4|a`, because those are distinct Cards. The mounted root displays one station and offers **Open source tree**. Its visible children in the host are exactly the children the host editor placed there. Self-mounting the root is already excluded by uniqueness. Even reciprocal root mounts are well-defined because rendering, locking, seeding, and exporting do not recursively expand portals. Back must restore the exact originating occurrence.

**One-time seeding.** Accept all three sources with an explicit order preview:

- **Hub → Reading tree.** Preserve group and item order in the draft. A Hub member description explains what a Card contributes to a group. It can become a proposed gloss, but it does not automatically explain why that Card follows its newly assigned predecessor.
- **Spread → Reading tree.** Let the reader specify the intended order. Table coordinates and stacking order do not establish an argument. Preserve the saved question when present. The current GUI already has a specialized Hub-draft path for a Spread kept from a question. [Existing Spread filing behavior](E:/zet/docs/wiki/gui.md:975)
- **Dependency Lens → Reading tree.** Use the bounded, evidence-backed selection described above. Record its source revision and selection parameters as provenance. All three operations preview duplicates and produce one placement per Card. None establishes continuing synchronization.

## 7. Decisions to carry into the final proposal

I consider the architecture **converged for drafting**. The remaining work is implementation qualification and reader testing. These four acceptance cases distinguish the agreed design from a superficially similar implementation:

1. **Topic evidence.** Mount an external Event beside `3/2`, where `3/2|a` is already occupied. Remove the mount. Every legacy home address, authored body, structural report, and Hub coverage result is identical before and after.
2. **Promotion and return.** Cite `[[~7#4]]` from a human Card, promote the cited claim into a same-Workspace Topic, and later delete its original Reading tree. The citation still opens the same Card without rewriting the human sentence. Until deletion of the reading, Back from the promoted claim restores the original station.
3. **Shared prerequisite.** Two theorems use the same definition, one through a proof step in another book. The lens shows one definition, both uses, and the exact carrier sentence for each. Seeding includes the definition once. A cycle or traversal bound is visible before save.
4. **Source retirement and drift.** An unlocked human reading survives an agent re-digest with its rationale, position, children, and missing target identity intact. Repeat with a Locked reading: deleting the mounted target is refused, an otherwise legal source-body edit succeeds, and the Reader can acknowledge the changed fingerprint without altering the original seal.

The contract I would lead with in the final proposal is **one Card, one home, one position per tree, and an identifiable author for every claimed relationship**. The Topic-side attachment, the historical Missing mount, and the math proof-step lens are where that contract earns its complexity. This round verified the cited source paths and shell tokenization in checkout `06cb56c8`; the proposed schema and acceptance cases have not been implemented or run.

---

## Post-Round-2 Code Verification Note (Gemini)

Before writing the final converged proposals, Gemini verified Astra's Round 2 source citations against the working checkout:

1. **`reseal` contract ([src/tree_lock.rs:6488](../src/tree_lock.rs#L6488)):** Confirmed. `trusted_reseal` is explicitly a test-only migration fixture and ZET exposes no public reseal command. The final proposal therefore adopts Astra's correction: use an append-only **`Review source` / `Mark reviewed`** observation record (`lock_source_reviews`) without modifying the Lock seal.
2. **`card::authored_fingerprint` coverage ([src/card.rs:96-109](../src/card.rs#L96-L109) & [src/main.rs:2646](../src/main.rs#L2646)):** Nuance caught and corrected. Astra noted in Round 2 that `authored_fingerprint` did not include `hub_level`; inspecting `src/card.rs:105` shows it passes `card.level_id()`, which in `src/main.rs:2646` returns `self.level.map(Level::id).or(self.hub_level.map(HubLevel::id))`. Thus `authored_fingerprint` **does** cover `hub_level` (as well as `model_level`), while excluding generated Reverse links, `literature_type` shelf labels, and external descendants of a mounted tree root. Section 8.1 of both English and Chinese proposals records this verified exact coverage.

---

## Round 3 — Gemini's Redesign Brief to Astra (Beyond "Reading Tree")
# Round 3 Co-Design Discussion: Beyond "Reading Tree" — A Free, General Working Tree for Card Relationships

The user reviewed our converged Rounds 1–2 proposal and gave us an important directional correction. Here is the user's exact request:

> "I want this new tree to be free enough for building relationship, so it should not be called 'reading tree', and help me generate a design for this direction. /ask-astra you two fully discuss this, this tree should reflect a general connection between cards so we can work on. please fully discuss on the design idea of this and update both englihs and chinese version."

This is a genuine paradigm upgrade, not just a find-and-replace rename. Let's step back from our "Reading tree" framing—which implicitly cast the tree as a *passive editorial sequence* ("how an editor leads a reader through existing material")—and re-examine from first principles of human comprehension and ZET's architecture what a **free, general-purpose tree for building and working on relationships between Cards** should be.

Please inspect any additional code paths in `E:\zet` that become relevant (`src/model.rs`, `src/hub.rs`, `src/gui/layout.js`, `src/tree_lock.rs`, `src/link_index.rs`) and give a sharp, candid Round 3 analysis on these six core design questions:

---

## 1. Naming (`CONTEXT.md` English + Chinese)

Why did "Reading tree" feel wrong to the user? Because "reading" implies a finished, passive consumption path rather than an **active space for discovering, building, structuring, and working on general connections between Cards** (literature clusters, mathematical dependency/working maps, historical evidence webs, research frameworks, project outlines, hypothesis trees).

Let's evaluate clean, non-jargony, `CONTEXT.md`-consistent names that do NOT collide with existing ZET terms (`Card`, `Link`, `Topic tree`, `Literature tree`, `Hub Card`, `Structure Card`, `Model Card`, `Model relation`, `Relation kind`, `Table`, `Spread`, `Folgezettel map`, `Constellation`):

- **Connection tree** (`关联树` / `连接树`) — directly mirrors "reflect a general connection between cards"; distinct from `Link` (`[[...]]`) and `Model relation`.
- **Relation tree** (`关系树`) — crisp, natural in Chinese (`关系树`), clearly says what the tree organizes; slight proximity to `Relation kind` / `Model relation` (arguably a strength or a slight overlap).
- **Network tree / Graph tree** — contradictory terms.
- **Working tree** (`工作树`) — collides with Git's "working tree".
- **Open tree** (`自由树` / `开放树`) — "free tree" already means an unrooted tree in graph theory, and "open" doesn't say *what* it connects.
- **Any better candidate you propose?**

Which name is sharpest, most durable, and clearest in both English and Chinese?

---

## 2. Tree Skeleton + Optional Tree-Local Cross-Connections (`placement_edges`)?

In Rounds 1–2, we assumed a tree contains ONLY parent-child tree edges (`Direct` / `Side`) and kept every non-tree relationship strictly inside Card bodies (`[[links]]`).

**Challenge to re-examine:** Look at what happens when a user is *actively working on connections* between Cards across Locked or foreign-Workspace trees:
- Literature group (`~7`): `Grossman1986` (`~7/1`) → `Hart1995` (`~7/2`) → `Holmstrom1999` (`~7/3`), AND the user wants to record right in `~7` that `Holmstrom1999` (`~7/3`) also directly challenges `Grossman1986` (`~7/1`)—**without editing either Locked Literature root!**
- Math working tree (`~12`): `Def D1` (`~12/1`), `Def D2` (`~12/2`), `Thm T` (`~12/3`), `Thm U` (`~12/4`), AND the user wants to connect `Thm T` to both `D1` and `D2`, and `Thm U` to `D1`, right on their working tree!
- History investigation (`~15`): `Event A` (`~15/1`), `Event B` (`~15/2`), `Finding C` (`~15/3`), AND `Finding C` corroborates both `Event A` and `Event B`!

**Proposed idea to stress-test: A Tree-Anchored Working Connection Space:**
1. **Primary Folgezettel Tree Skeleton (`placements`):** Every Card in the tree gets a clear Folgezettel coordinate (`~7/1`, `~7/2|a`), preserving working memory, linear focus lens (`ZtLayout.outline`), bounded branching, and zero-hairball navigation.
2. **Optional Tree-Local Cross-Connections (`placement_edges`):** Between any two placements `(from_placement_id, to_placement_id)` in the **same tree**, the tree's owner can record an explicit directed connection with a relationship label and short note—stored on the **host tree**, so you can connect two Locked or other-Workspace Cards freely while working!
3. **Clear epistemic distinction in the UI:**
   - **Card-body authored Links (`[[...]]`)** remain the durable, box-wide semantic assertions (shown as solid/canonical threads, feeding `Reverse links`, `Dependency Lens`, and `Model`/`Math` reports).
   - **Tree-local connections** (both parent-child placement edges and optional cross-edges) belong to *this working tree* and its Workspace owner—visible right on the tree's Map (cross-edges shown cleanly on focus/selection or toggle so the Map never turns into a hairball!), never corrupting global Model/Math/Term reports, and optionally **graduatable** into a permanent authored Link or native synthesis Card when the user's thinking solidifies!

**Push back hard on this!** Does adding `placement_edges` genuinely unlock "free enough for building general relationships to work on" while preserving first principles of human comprehension—OR does it create a two-tier link confusion where users don't know whether a connection is "real" (`[[...]]`) vs. "local to `~7`"? How do we make the distinction cognitively effortless (or should we design it differently)?

---

## 3. Freedom of Edge Labels & Notes (Beyond 8 Fixed `RelationKind`s?)

In Rounds 1–2, we limited a placement edge to:
- `relation_kind`: strictly one of ZET's 8 fixed kinds (`rests-on`, `illustrates`, `contrasts`, `relaxes`, `corresponds`, `objects`, `specializes`, `refers`)
- `gloss`: max 240 chars, plain text, no Markdown Links.

Now think about a researcher *freely building relationships* across economics, math, history, and personal projects:
- What if they want domain-specific or working relationship labels like `因果线索`, `同题竞争`, `前置定义`, `时间先后`, `子问题`, `共同假设`, `待验证`, `parallel case`, `mechanism`, `precursor`?
- Should an edge support:
  - an optional **canonical `relation_kind`** (the 8 ZET kinds, for filtering/coloring/interoperability) **AND/OR** a free short **`relation_label`** (e.g., 1–32 chars, one line, just like ZET's `Model facet`! Look at `CONTEXT.md`: Model facets are free 1–32 char author labels with common spellings normalized!)?
  - What about the **edge note** (rationale)? Should it stay bounded plain text (say, up to ~280–400 chars, no `[[links]]`) so anything richer becomes a real citable native Card—OR is there any case where an edge needs more?
- Also: what about **unlabeled / quick-connect edges** when brainstorming? When working freely, a user often mounts 6 Cards first to group and order them, and only labels/explains the relationships *after* seeing them together! So `relation_kind`, `relation_label`, and `note` MUST all be optional at creation time (zero friction while working!).

---

## 4. Freedom of Native Card Types Inside the New Tree

In Rounds 1–2, we restricted native Cards in `~7` to ordinary `idea` Cards and kept `event`, `finding`, `model`, `hub`, `term`, `cited` strictly in Literature/Topic homes.

**Stress-test this restriction!** If a user works actively inside a general connection tree `~7`:
- Why shouldn't they be able to create a native **`event`** or **`finding`** Card (e.g., a newly discovered primary-source event or empirical check during an investigation)?
- Why shouldn't they be able to create a native **`model`** Card (a new synthesis framework or original theoretical model grown directly out of comparing Hart and Holmstrom in `~7`, possibly with native Model parts filed under it)?
- Wait — **check the codebase (`src/model.rs`, `src/term.rs`, `src/cited.rs`, `src/hub.rs`) carefully!**
  - In `src/model.rs`, `find_head`, `build_report`, and `blocks_report` currently walk `parent_location` and sort by `address_order` on positional `location` strings!
  - If a Model Card (`~7#1`) and its Model parts (`~7#2`, `~7#3`) are native to `~7`, their canonical handles (`~7#1`, `~7#2`, `~7#3`) do NOT encode parent-child structure—their **placements in `~7`** (`~7/2`, `~7/2|a`, `~7/2|a|1`) DO!
  - Can `home_parent(card_id)` cleanly work for a `~7`-homed tree by reading `~7`'s own placements (so a native Model Card + native Model parts inside `~7` work seamlessly, while mounted foreign parts still use *their* own home's `home_parent`)—OR does allowing native Model parts / Hubs inside `~7` introduce subtle traps when someone reorders `~7` (e.g., dragging a Model part out from under its Model Card or dropping a mounted Card in between)?
- Please check `src/model.rs` and tell us honestly: which native Card types should be **free from Day 1** (`idea`, `event`, `finding`, standalone `model`?), which need explicit structural rules (multi-part `model`s, `hub`s), and which genuinely belong strictly to Literature trees (`term` book lists, `cited` reference lists)?

---

## 5. Topology Freedom: Coordinates, Uniqueness (`UNIQUE(tree_id, card_id)`), and Root Branching

Let's double-check two structural assumptions from Rounds 1–2 under the new "free working tree" lens:

1. **Root branching (`~7/0`):**
   - In `ZtLayout.outline` (`src/gui/layout.js:56`), a tree root's `Direct` chain (`~7/1`, `~7/2`, `~7/3`) ALREADY renders as the top-level outline siblings (`depth = 1`), and each item's `Side` branches (`~7/1|a`, `~7/1|b`) render as its children (`depth = 2`).
   - Question: Is keeping `~7/0` → Direct spine (`~7/1`, `~7/2`, `~7/3`) with arbitrary `Side` branching from `~7/1` onward already completely free for grouping parallel lines (where `~7/1`, `~7/2`, `~7/3` are top-level clusters/papers/phases, NOT forced to imply linear causation unless an edge label says so), OR should `~7/0` itself allow `Side` children? (Check `layout.js` and `place.rs` implications!)
2. **Repeated appearances vs. `UNIQUE(tree_id, card_id)`:**
   - Re-verify: If a tree supports **tree-local cross-connections (`placement_edges`)** (or companion-pane recall), is `UNIQUE(tree_id, card_id)` STILL strictly superior to allowing the same Card to occupy two coordinates in the same tree? Or is there a compelling working-tree use case where one Card *must* sit at two coordinates in the same tree?

---

## 6. Overall System Coherence (Topic Mounts, Dependency Lens, Working Actions)

Under this freer, active-working-tree paradigm:
1. What **working operations** make this tree feel like a genuine *workbench* rather than a static list? (E.g., quick-mount search, drag/reparent with gloss review, add tree-local cross-connection, "Open subtree on Table" as a Spread, "Graduate tree-local connection into authored `[[link]]` / native Card", "Split branch into a new tree", "Promote native Card to Topic", "Seed from Dependency Lens / Hub / Spread / Search results").
2. Should **Topic-side mounts (`3/2+1`)** stay in the design as Phase 4 (lightweight evidence attachment on Topic Cards), and does the **Cross-Book Dependency Lens** still stay a first-class sibling feature (especially for math)?
3. What invariants from Rounds 1–2 MUST remain rock-solid non-negotiables (e.g., canonical single Home, `~7` / `~7#4` permanent handles vs. `~7/2|a` mutable coordinates, Missing mounts via `card_identity` retirement, Lock source-drift observation + `Mark reviewed`, zero-rewrite reordering, strict `home_parent` invariance)?

Let's debate this thoroughly so our updated English and Chinese proposals reflect a truly liberating, rock-solid, first-principles design!
---

## Round 3 — Astra's Decision & Revision Memo (`gpt-6.2-astra`, high)
Command exited with code 0.

Command output:
# Round 3 decision — Connection tree / 关联树

Astra and I recommend **Connection tree / 关联树**. The decisive change is that the tree’s author can **record a relationship between two Cards without changing either Card or putting one beneath the other**. Selecting and arranging Cards is useful on its own. Drawing a connection is a separate act. Publishing a claim in a Card is a third.

Astra initially preferred *Relation tree* and raised a fair objection to *Connection tree*: “connection” can mean mere co-occurrence. We settled on **Connection tree** because it accommodates both an unexplained working association and a precisely stated relationship. The interface and glossary must distinguish those states. **关联树** is preferable to **连接树** for the same reason. It describes intellectual association across materials. **关系树** is natural Chinese, but sits closer to ZET’s existing “模型关系” and “关系类型” than this broader concept warrants.

I inspected the existing English and Chinese proposals and the relevant Rust and GUI paths in checkout `06cb56c8`. **This session has read-only filesystem access.** The two files still contain Round 2. The English and Chinese revisions below give the agreed design in Markdown. [Existing English proposal](E:/zet/docs/multi-tree-coordinates-proposal.md) · [现有中文方案](E:/zet/docs/multi-tree-coordinates-proposal.zh.md)

---

# English proposal — Connection trees in ZET

## 1. What the tree is for

> **Connection tree.** An authored tree in which a person or agent selects Cards, arranges them, and records how particular Cards relate. It can contain Cards mounted from elsewhere and Cards written in the tree. Each Card occupies at most one position. A connection between two positions belongs to the tree that records it.

A researcher can begin with a title, six Cards, and no settled question. They can put two papers beside each other, notice that a third challenges the first, record that observation, then rearrange the papers when a better organizing principle emerges. The observation should survive the rearrangement. A system that requires a finished reading order, a relation type, or a defensible claim before the first connection misses the moment the user wants to capture.

“Reading tree” described one eventual use of the result. “Working tree” leaves the subject unspecified and collides with Git. “Open” describes a permission or topology. “Network tree” and “Graph tree” leave it unclear which structure determines position. **Connection tree names the work.** A reading sequence, a hypothesis tree, a historical investigation, and a project outline can all use it without a new tree type.

## 2. Three records with different meanings

| Record | Question it answers | Effect of moving a Card to another branch |
|---|---|---|
| **Placement and optional gloss** | Where did I put this Card? Why does it belong at this point? | The occurrence follows the move. A change of structural predecessor requires review or clearing of the affected nonempty glosses. |
| **Tree connection** | What association or assertion did this tree’s editor record between these two Cards? | It follows the two placement identities. Coordinates can change without changing the connection. |
| **Card Link** | What does this Card’s author say about another Card in this passage? | Its meaning comes from the authored passage and existing Link rules. The connection tree can show the passage and its provenance. |

**Allow tree connections between any two distinct placements in the same Connection tree, including adjacent placements and the root.** This last detail removes a trap in Round 2. If Hart follows Grossman, the gloss might say “Compare the assignment of control rights here.” An explicit connection might say “Hart extends Grossman under these assumptions.” Moving Hart changes the first statement’s context. It leaves the second statement’s endpoints intact. Storing substantive relation kinds on the incoming placement would give the editor two places to enter the same assertion with different move behavior. Keep the positional gloss there and put substantive labels on `placement_edges`.

Make provenance visible in words at the point of use. The map can show **“In this tree · recorded in ~7”** or **“In the Card · Book B, proof step 3”**. Selecting the former opens the tree editor. Selecting the latter opens the authored passage. A local connection can be carefully researched; an authored Link can be speculative. Line style and color may distinguish their origins, but should never imply a ranking of truth.

Three examples establish the boundary:

- **Literature.** Place Grossman, Hart, and Holmstrom in whatever order serves the investigation. Record `Holmstrom → Grossman`, labelled “challenges,” even when both Literature roots are Locked. The user is responsible for that interpretation. Opening either book and returning restores the exact placement.
- **Mathematics.** Place `D1`, `D2`, `T`, and `U` once each. Draw `T → D1`, `T → D2`, and `U → D1`. Selecting `D1` exposes its shared use. The separate Dependency Lens can show which uses have an authored proof-step sentence, which appear only in the working tree, and which occur in both. Only the authored evidence enters the computed prerequisite graph.
- **History.** Connect Finding C to Events A and B. The two connections can carry different notes about what C corroborates. If the argument depends on A and B *jointly*, write that argument on a synthesis Card. Two pairwise lines alone cannot express joint necessity or establish independence of the sources.

## 3. Connection grammar and storage

**Quick connect creates a neutral pair.** Direction, label, kind, and note are optional. Clicking A and then B does not assert that A causes, precedes, or depends on B. When the editor chooses a direction, display it as a sentence such as **“T depends on D1.”** This matters for math because prerequisite-first reading travels in the opposite direction. An undirected association can also mean “compare these”; it need not declare a formally symmetric predicate.

| Field | Proposed rule |
|---|---|
| Identity and endpoints | Immutable `connection_id`; host `tree_id`; two distinct placement IDs. Both endpoints must belong to that Connection tree. |
| Direction | Neutral or directed. Normalize endpoint order for a neutral pair. Preserve author-selected source and target for a directed connection. |
| `relation_label` | Optional free label, one line, 1–32 Unicode scalar values. Keep the author’s spelling. Suggest labels previously used in this tree. |
| `relation_kind` | Optional explicit classification using the existing eight ordinary relation names. `term` remains reserved for Term-link syntax. A local classification participates only in local filters. |
| `note` | Optional plain text, up to 400 Unicode scalar values. The positional gloss retains its separate 240-character limit. Neither field parses `[[…]]` or creates Links. |
| Authorship | Record the creating and last-editing actor available to ZET and the revisions. Display the host Workspace separately from the owner of either Card. |

`共同假设`, `时间先后`, `待验证`, `parallel case`, and `mechanism` are all valid working labels. Do not translate them automatically into `corresponds`, `rests-on`, or any other kind. `待验证` may describe the editor’s uncertainty rather than a predicate between the papers. A directional canonical classification such as `rests-on` requires the editor to choose which endpoint rests on which. The existing Model facet is a precedent for short author-defined labels; the global Link kind has a different contract and is derived from authored wording. [Model facets](E:/zet/src/model.rs:33) · [Relation kinds](E:/zet/CONTEXT.md:54)

Use composite foreign keys from each `(tree_id, placement_id)` endpoint to the placement table, with indexes for incoming and outgoing lookup. Keep one completely blank quick connection per unordered pair. Permit distinct explicit assertions about the same pair: paper A can borrow B’s definition and dispute B’s empirical interpretation. Display these as two selectable statements. Use request idempotency for retries rather than a pair-wide uniqueness constraint that would discard the second assertion.

`placement_edges` is authoritative authored data. `links` is a rebuildable occurrence index over Card bodies. Putting both in `links` would make `zt reindex` either erase editorial work or recreate assertions nobody wrote in the Cards. Reuse the renderer and relation vocabulary where useful. Keep the stores, authority checks, and audit paths distinct. [Link-index schema and rebuild](E:/zet/src/link_index.rs:159)

## 4. Native Card types and home structure

| Type | Decision |
|---|---|
| **Idea, Event, Finding** | Support all three in the first release with native authoring. An Event can originate in an investigation. A Finding can record a new empirical check. Keep the type-specific fields, source requirements, and applicable gate policy. |
| **Standalone Model head** | Allow an ordinary concept Model head once create, edit, and `get_model` work end-to-end for its permanent handle. An unfinished framework can begin as an Idea immediately. A facetless head can still acquire members through authored Links, so an empty model is insufficient as a qualification test. |
| **Multi-part Models, mathematical units, Theory Cards** | Allow mounting from day one. Enable native structured authoring after explicit home ancestry and ordering work across membership, imports, board layout, math reports, gate checks, and promotion. |
| **Hub** | Allow mounts and eligible native claims as Hub destinations. Keep native `line`, `topic`, `field`, and `register` Hubs in the home contexts that give their levels and coverage meaning. |
| **Term and Cited** | Mount the existing Cards freely. Keep the book-specific list, row, head, and continuation families in Literature homes. An original definition can be an Idea or, once supported, a mathematical unit. It need not masquerade as a book’s Term entry. |

For Events and Findings, **source and home are different facts**. A Finding created in `~15` can cite Book A, an archival document, or an explicitly described empirical check. Passing `~15` to a validator as its “book” would give it false provenance. The current gate derives book context from a tree string, and the Finding rules distinguish a known book from a general source line. Supply typed home-role and actual source context. A local corroboration line cannot satisfy a rule that asks for an incoming authored Idea Link. [Gate context](E:/zet/src/gate.rs:909) · [Finding rules](E:/zet/tools/zt-check/src/core.rs:2686)

**`home_parent(card_id)` alone does not solve native Models.** `find_head` walks through Model parts and stops at an intervening non-Model Card. `parts_of` also filters by tree membership and sorts by Folgezettel address. The board uses address-prefix tests to group blocks. A permanent handle such as `~7#4` encodes none of that structure. Simply substituting a placement lookup for `parent_location` would make inserting a mounted paper between a head and part silently change the model. [Head lookup](E:/zet/src/model.rs:278) · [Filed and linked parts](E:/zet/src/model.rs:1028) · [Board grouping](E:/zet/src/model.rs:1470)

My recommendation for native structured families is to give them an explicit **home parent, sibling order, and home-ancestry query**. The work map may initially display the same arrangement. **Move on map** edits placements. **Refile model part** previews changes to home membership and reports, then runs the gate. A mounted foreign part always retains its own home. Existing model membership through authored Links and `**引入**` continues under its existing rules. This permits free rearrangement without turning an ordinary drag into an unannounced change to a proof.

## 5. Topology, working actions, and lifecycle

**Retain one Direct successor of the root and one placement per Card per tree.** The outline already presents `~7/1`, `~7/2`, and `~7/3` as top-level peers. They can be competing explanations, phases, or clusters. Their order gives the map a stable traversal and carries no causal implication. Root Side branches would introduce a second spelling for comparable top-level organization and require coordinated changes to outline, indent, and outdent. Existing outline recovery for malformed input is no basis for a new grammar. [Outline construction](E:/zet/src/gui/layout.js:30) · [Indent and outdent](E:/zet/src/place.rs:360)

The strongest case for repeated placement is a paper playing two roles in two arguments. Put the **paper** in the tree once. Give each argument its own native claim or context-specific Card and connect both to the paper, with the relevant passage or explanation. A companion pane makes the paper readable at either argument. The reader can now see both roles **and that they rely on the same work**.

| Working action | Contract |
|---|---|
| **Gather and connect** | Search, multi-mount, reorder, connect two placements, then label later. Selecting a Card shows its incoming, outgoing, and neutral connections. The default map shows focused connections, with counts and a list for dense neighborhoods. |
| **Inspect together** | Peek at the endpoint beside the selected Card. Open a selected branch on the Table. Saving a Spread captures a chosen arrangement; it creates no new claims. |
| **Develop an assertion** | Open a draft Link sentence in a selected writable carrier, or create a native synthesis Card referring to both endpoints. Saving follows ordinary scope, Lock, and gate rules. A synthesis that cites A and B supplies its own argument; it does not fabricate an authored `A → B` Link. Let the editor retain or remove the local connection explicitly. |
| **Extract a branch** | Default to **copy into a new tree**. Copy the selected placements and internal connections with fresh IDs. Keep the original and its crossing connections intact. Preview boundary connections; never silently mount the other endpoints. A destructive split is a separate planned operation. |
| **Remove or replace** | Detaching a placement previews its children, glosses, and incident connections. Retiring the source Card instead leaves a Missing mount **and its original connections**. Rebinding to a different Card requires explicit decisions about those assertions. They cannot become evidence about the replacement by inheritance. |
| **Seed and continue** | Seed from a Hub, Spread, Dependency Lens, or selected search hits. Preview order and selection. A search rank or two adjacent seed items supplies no connection. Promote eligible native Cards to a Topic within the same Workspace while preserving their handles and original placements. |

Locks freeze the host’s structure, glosses, native content, **and connection records** under a versioned seal. Mounted targets retain their identity/address pins. Edits allowed at a target’s home can produce a source-drift observation; **Mark reviewed** appends an acknowledgement. It does not reseal the tree. A local edge adds no transitive lock on a target’s neighbors. `zt dp` must preserve connection identities and evidence. Portable export must distinguish an included local assertion, a Missing mount, and an external Card omitted from the package. [Existing Lock plan and seal](E:/zet/src/tree_lock.rs:1637)

**Release order.** Deliver mounting, local connections, focused map inspection, Missing mounts, and versioned dump/export together. Include a citable root and its resolver in that first slice. Add native authoring, promotion, and versioned host Locks next. Keep the cross-book Dependency Lens first-class, with exact proof-step evidence and a companion pane. Keep `3/2+1` Topic-side evidence attachments as Phase 4. They occupy their own slots beside Topic Cards and do not consume `3/2|a`, affect Hub coverage, or qualify as grown claims. Structured native Models have their own qualification gate.

I would also correct two details in the existing draft schema: explicitly require `ordinal IS NOT NULL` where an ordinal is mandatory, since a SQLite `CHECK` can accept `NULL`; and give `~n` a never-reused allocator independent of the Topic counter, which the current whole-archive deletion resets. [Draft placement schema](E:/zet/docs/multi-tree-coordinates-proposal.md:235) · [Topic counter reset](E:/zet/src/main.rs:4576)

---

# 中文方案 — ZET 关联树

## 一、用途与命名

> **关联树**：由人或智能体选取卡片、安排位置、记录卡片之间关联的一棵树。树中既可挂载已有卡片，也可创建原生卡片。同一张卡在每棵树中最多占一个位置。两个落点之间的关联由记录它的这棵树持有。

用户可以只起一个标题，先放进六张卡，不必预先写好研究问题、阅读顺序或关系说明。比较几张卡之后，作者可能发现第三篇论文反驳了第一篇，再把它们移到不同分支。**位置可以变，已经记下的这条反驳仍应指向原来的两篇论文。** 这是本轮修改相对于“阅读树”最重要的能力。

采用 **Connection tree / 关联树**。“阅读树”侧重成果怎样被阅读。“工作树”没有说明工作对象，也容易让人想到 Git。“关系树”本身自然，但与既有的“模型关系”“关系类型”过于接近。“关联”既容得下刚发现、还说不准的联系，也容得下方向和依据都很清楚的判断。界面应把这两种状态交代清楚。

## 二、位置、树内关联与正文链接

| 记录 | 它回答的问题 | 换分支后的处理 |
|---|---|---|
| **落点及可选按语** | 这张卡放在哪里？为什么放在这里？ | 落点随移动保留。实际前驱改变时，须复核或清除受影响的非空按语。 |
| **树内关联** | 这棵树的作者认为这两张卡之间有什么联系？ | 按两端的落点身份保存。仅坐标改变时，关联原样保留。 |
| **卡片正文链接** | 卡片作者在这句话、这个证明步骤中怎样使用另一张卡？ | 以正文及既有链接规则为准；可以从图上打开原句核对。 |

**同一棵关联树中的任意两个不同落点都可以建立关联。** 两点可以相邻，也可以相隔几个分支，树根同样可以参加。比如 Hart 紧接 Grossman 时，按语可以写“在此比较两文如何分配控制权”；若作者认为 Hart 在特定假设下扩展了 Grossman，应另记一条明确的树内关联。移走 Hart 后，前一句要复核，后一句仍对应原来的两篇文章。落点保存位置按语，有实质含义的关系标签统一存在关联记录中。

界面要直接显示出处，例如 **“本树记录 · ~7”** 和 **“卡片正文 · 乙书，证明第 3 步”**。点击前者编辑本树记录，点击后者查看原句。两者区分的是作者、出处和修改权限。一条树内判断也可以有充分依据，一条正文链接也可能只是猜想。

- **文献比较。** 将 Grossman、Hart、Holmstrom 排到合适的位置。即使两棵文献树均已锁定，作者仍可在自己的树中记录“Holmstrom 质疑 Grossman”。读者能进入原书，再回到原来的落点。
- **数学研究。** `D1`、`D2`、`T`、`U` 各放一次，记录 `T → D1`、`T → D2`、`U → D1`。选中 `D1` 即可看出两条定理共用它。打开依赖视图，可核对哪些用法有正文证明步骤支持，哪些只是本树作者的判断。计算前提闭包时只使用符合规则的正文证据。
- **历史考证。** 发现卡 C 可以分别关联事件 A、B，两条说明各写它支持哪一点。如果结论依赖 A、B 两份材料的共同成立，应写成综合卡。两条连线本身不能证明两份史料相互独立，也不能表达一项完整的联合论证。

## 三、怎样自由地建立关联

**快速关联默认没有方向。** 依次点选 A、B，只表示作者想把两者联系起来。需要方向时，由作者选定，并显示完整语句，例如“定理 T 依赖定义 D1”。数学学习时往往先读 D1 再读 T，界面尤其需要把这两个方向区分开。

每条关联有不可变的 `connection_id`、宿主树和两个落点。方向可选。`relation_label` 可填一行、1 至 32 个 Unicode 标量值的自由标签，保留作者用词，如“共同假设”“因果线索”“待验证”。`relation_kind` 可从既有八个普通关系名称中显式选取，仅用于树内筛选；`term` 保留给术语链接。选择 `rests-on` 这类有方向的分类时须指定两端。“时间先后”不能自动转换成因果或依赖。“待验证”可以只是留给自己的提醒。`note` 可留空，也可写至多 400 个 Unicode 标量值的纯文本。落点按语保留 240 的上限。两者均不解析 `[[…]]`。

每一对落点只需一条完全空白的快速关联。说清含义后，同一对卡可以有多条判断。例如，一篇论文沿用了另一篇的定义，同时质疑它的实证解释。重复提交请求用幂等键处理，不能用“两点之间只能连一次”的约束吞掉第二项判断。

`placement_edges` 保存作者写下的树内关联。两个端点均以“树 ID、落点 ID”复合外键校验，按两端建索引。现有 `links` 则由卡片正文推导，允许重建。执行 `zt reindex` 后，树内关联应完整保留，正文反向链接、术语使用次数和模型报告也不应凭空增加。

## 四、哪些卡片可以直接建在树里

| 类型 | 决议 |
|---|---|
| **想法卡、事件卡、发现卡** | 在首个支持原生写卡的版本一并开放。保留各类卡的字段、来源要求和写入检查。新发现的史实或实证结果可以直接产生于调查过程。 |
| **独立模型头卡** | 永久句柄贯通创建、编辑、`get_model` 完整读写链路后，开放普通概念模型头卡。验证要包括通过正文链接加入的模型部件，不能只测空模型。尚在形成的框架可先写为想法卡。 |
| **多部件模型、数学单元、理论卡** | 已有卡从第一阶段即可挂载。原生创建须等主树父子关系、排序、模型引入、模型板、数学报告和提升操作都能正确处理后再开放。 |
| **枢纽卡** | 可以挂载已有枢纽；有实质内容的原生卡也可成为枢纽入口。`line`、`topic`、`field`、`register` 的原生创建仍遵循各自的主树位置和覆盖规则。 |
| **术语卡、引文卡** | 可以自由挂载。某书的术语表、术语头卡及续卡、参考文献表及其条目继续由该文献树持有。作者自拟的概念可写为想法卡，或在结构支持就绪后写为数学单元。 |

**来源与归属必须分开。** 在 `~15` 中建立的发现卡，可以引用甲书、档案材料或作者自己的实证检验。不能为了通过检查就把 `~15` 当作书目来源。现有发现卡规则区分“已知所属文献”和“一般来源说明”。写闸门应取得真实的主树角色及来源上下文；树内一条“印证”连线也不能冒充规则要求的正文链接。

**只增加一个 `home_parent(card_id)` 还不够。** 现有模型代码会沿模型部件向上寻找头卡，遇到普通卡即停止。收集部件时还检查地址所属树、按地址排序，模型板又用地址前缀判断块内层次。`~7#4` 是永久引用句柄，不包含这些结构信息。若直接沿工作图上的落点找父卡，在模型头和部件之间插入一篇挂载论文，就可能悄然改变模型归属。

原生结构化模型应具有明确的**主树父节点、同级顺序及祖先查询**。初次显示时可以与工作图一致。**“在图上移动”**只改落点；**“调整模型部件归属”**预览成员与报告变化后，经过写闸门提交。挂载进来的外部部件始终按自己的主树计算。正文链接和 `**引入**` 原有的成员与引入语义继续生效。

## 五、拓扑、工作操作与发布条件

**树根保留一个直接后继，每张卡在同一棵树中保留一个落点。** 现有大纲已将 `~7/1`、`~7/2`、`~7/3` 显示为同级项目，可以分别代表几个研究阶段或互相竞争的解释。编号表达遍历次序，不自动表达因果。树根再设侧支，会给相同的顶层组织引入第二套写法，还要同时修改缩进和提升层级的规则。

一篇论文在两条论证中承担不同作用时，论文放一次，两条论证分别设卡，各自说明所用段落及其作用。任一论证都可以侧栏查看原文。这样既保留两种用法，又能直接看出它们依赖同一份材料。

| 工作操作 | 行为 |
|---|---|
| **收集与关联** | 搜索、多选挂载、拖动排序、先连线后命名。选中落点可查关联的方向和注释。图上默认显示与焦点有关的连线；密集区域给出数量和可展开清单。 |
| **对照查看** | 侧栏并排查看另一端，或把选中分支摊到 Table 上。保存 Spread 只记录摆放，不产生判断。 |
| **展开为论证** | 在有权编辑的卡中起草带链接的句子，或新建综合卡引用两端。保存遵循正文的权限、锁和写闸门。综合卡引用 A、B，不等于 A 的作者写了指向 B 的链接。原树内关联是否保留，由作者决定。 |
| **抽出分支** | 默认复制为新树，复制选中落点及两端都在所选范围内的关联，分配新 ID。原树及跨边界关联完整保留。预览列出边界另一端，不自动拉入新树。破坏性拆分另行计划确认。 |
| **解除挂载与替换** | 解除挂载前列出子节点、按语及相关连线。若删除的是来源卡，则保留缺失挂载及其原有关联。换绑另一张卡时逐项决定这些判断的去留，不能把旧证据自动转给新卡。 |
| **建树与提升** | 可由枢纽、Spread、依赖视图或所选搜索结果一次性建树，提交前查看顺序。搜索排名及相邻位置都不产生关联。同工作区的合格原生卡可提升到主题树，永久句柄与原落点继续有效。 |

**身份与锁的约定继续成立。** 一卡一个主树。同树至多一个落点。`~7` 引用树根，`~7#4` 永久引用卡片，`~7/2|a` 是可变位置，`place:<uuid>` 标识某次放置。普通重排不改写正文。锁定关联树时，位置、按语、原生正文和树内关联都纳入版本化的锁证据；挂载目标沿用身份与地址保护。源卡在其主树中发生合法编辑时可以提示版本差异，**“标为已核阅”**追加审阅记录。锁不会沿关联递归冻结其他卡。完整备份须保存关联及其证据；便携导出须区分“本包内的树内判断”“来源卡已删除”和“该卡未随包导出”。

**交付顺序。** 第一阶段一起交付可引用的树根、挂载、树内关联、焦点查看、缺失挂载，以及可往返验证的备份和导出。第二阶段开放原生写卡、提升和新版树锁。跨书依赖视图保留为独立功能，展示引用它的实际证明步骤。`3/2+1` 主题侧挂保留到第四阶段，不占用既有 `3/2|a`，也不计入枢纽覆盖或想法生长成果。原生结构化模型单独验收。落地时还须补严两处约束：必须排序的落点显式要求 `ordinal IS NOT NULL`；`~n` 使用永不复用的独立分配器。

**最终验收用一个连续动作。** 在自己的关联树中挂载两张已锁定的文献卡，写下质疑，移动其中一张，再从证明或史料原文返回。作者应该能看见：位置按语已提示复核，质疑仍指向原来的两张卡，两篇原文和全库依赖报告均未变化，返回后仍能接着工作。这比界面上出现一个“关联树”菜单更能说明设计是否真正满足用户的要求。
---

## Post-Round-3 Code Verification Note (Gemini)

Before updating both English and Chinese proposals to the converged **Connection tree / 关联树** design, Gemini verified Astra's Round 3 source citations against checkout `06cb56c8`:

1. **Gate book-context inference ([src/gate.rs:909](../src/gate.rs#L909)):** Confirmed (`facts.book = zt_check::text::literature_tree(&facts.tree);`). Creating Events or Findings natively inside `~n` genuinely requires separating typed `TreeRole` and actual source context from the Home tree string.
2. **Whole-archive delete counter reset ([src/main.rs:4576-4595](../src/main.rs#L4576-L4595)):** Confirmed (`apply_delete_all_plan` resets `next_topic_id` while retaining ended Lock history and receipts). Giving `~n` an independent never-reused `next_connection_id` allocator prevents handle collisions.
3. **Model head ancestry lookup ([src/model.rs:278-291](../src/model.rs#L278-L291)):** Confirmed (`find_head` walks `parent_location(&address)` through Model parts and stops at any non-Model Card). Enabling native multi-part Models in Connection trees therefore requires an explicit `home_structure` table separate from working-map `placements`.
4. **SQLite `CHECK` null semantics:** Confirmed (`CHECK(ordinal >= 1)` passes `NULL` in SQLite). Updated `placements` DDL in both proposals to explicitly require `ordinal IS NOT NULL AND ordinal >= 1` for `side` and `attach` edges.
