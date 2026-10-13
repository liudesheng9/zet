# ZT Note System

ZT is a local note system built around addressable Markdown cards, with terminal navigation and a local GUI.

## Language

**Card**:
A note item in the system. A Card is one Markdown document: a `# Title` heading, a Markdown body, and a generated Reverse link section after the `<!-- zt:reverse-links -->` marker. It has a unique address.
_Avoid_: Note item, note record

**Regular Card**:
A non-root Card in a Topic tree or Literature tree.
_Avoid_: Normal Card

**Card type**:
What a Card holds: `idea` (the default), `event`, or `model`. Only Regular Cards can be Event or Model Cards; Topic and Literature Cards are always Idea Cards. The type stays with the Card when it moves.
_Avoid_: Card kind (kind means Topic, Literature, or Regular), tag, category

**Idea Card**:
A Card of type `idea`: one idea or claim, in the Card's own words. Most Cards are Idea Cards.

**Event Card**:
A Regular Card of type `event`: it records one event that happened in the real world (what, when, where, who, and the source that reports it), not an idea or an argument. Idea Cards that use the event as evidence or illustration link to it.
_Avoid_: Fact card, case

**Model Card**:
A Regular Card of type `model` without a Model facet: the entry point of one whole model. It states the model's question and what the model shows; it may list the model's parts, and a Link from it to a Model part takes that part into the model. Every Card that derives, interprets, extends, or criticises the model links to it or to the part it concerns, so their Reverse links list the discussion of the model.
_Avoid_: Theory card, formula card, model header

**Model part**:
A Regular Card of type `model` with a Model facet. It presents one part of a model in depth and belongs to the Model Card it is filed under (as a successor of the Model Card or of another of its parts), if any, and to every Model Card that links it. So a part may be filed wherever it first connects, and one part may serve several models; a part in no model is reported. A Model Card with its parts is one **model**; which parts it has, what they are called, and how they relate is decided by whoever writes the model. ZT computes the structure from Locations, facets, and Links.
_Avoid_: Sub-card, child model, model section

**Model facet**:
What a Model part presents in its model: a short label (at most 32 characters, one line) chosen by the model's author, such as `设定`, `均衡`, `数值例子`, `a worked example`. There is no required set or order of facets; `setting`, `assumptions`, `timing`, `dynamics`, `solution`, `results`, `intuition`, and `limits` are only stored in one spelling. A model may hold several parts with one facet.
_Avoid_: Role, section, aspect, slot

**Emphasis**:
How much of a reader's attention a Model part asks for: `core` (the heart of the model, what a reader must see first), `aside` (detail a reader may skip), or none (a regular part). Only Model parts have one; the GUI spends ink and space by it.
_Avoid_: Priority, weight, importance

**Model relation**:
A Link from one Model part to another part of the same model. The sentence that holds the Link is the relation's reason, and its words give the relation a Relation kind. A model's relations need not form a sequence or a tree, and they are separate from where the parts are filed.
_Avoid_: Dependency edge, arrow

**Relation kind**:
What a Link says about the Card it names, read from the words its author wrote before it in the same sentence: `rests-on` (`依赖`, `基于`, `代入`, `推导自`, `rests on`, `uses`), `illustrates` (`例示`, `案例`, `for example`), `contrasts` (`对照`, `区别`, `unlike`), `relaxes` (`放宽`, `推广`, `extends`), `corresponds` (`对应`, `类比`, `maps onto`), `objects` (`反驳`, `批评`, `counterexample`), or `refers` when no such word stands before it. A Link right after another in the same sentence shares its kind. Kinds are computed, never stored; only `rests-on` places a part on the Model board.
_Avoid_: Link type, edge label

**Loose part**:
A Model part in no Model relation: it links no other part and no part links it, such as a historical case or a variant set beside the model.
_Avoid_: Orphan, detached part (a detached part is one that belongs to no model)

**Model neighbour**:
A Card outside a model that links one of its Cards or is linked by one, with the Relation kind and sentence of that Link, such as an Idea Card that relaxes an assumption or another model a part corresponds to. Tree roots are not neighbours.
_Avoid_: Discussion card, external part

**HTML chart**:
HTML in a Card body that the GUI shows as a figure: a ```` ```chart ```` fence, or a raw HTML block that opens with a chart element such as `<figure>`, `<div>`, `<svg>`, or `<table>`. It runs sandboxed, apart from the GUI and its token; it may draw with scripts, use TeX, and name Cards with `[[address]]` or `data-zt="<address>"` to let a reader open them. Text inside a chart fence is not a Link.
_Avoid_: Widget, embed, diagram card

**Artifact**:
An image a Card shows: a figure, table, or cover taken from a Scholar Archive Work's Markdown. A Card shows it with `![caption](artifact:<sha256>)`, where the address is the SHA-256 of the image's bytes. ZET keeps the bytes once, in the archive's `artifacts/` folder, beside a record of where they came from; an artifact belongs to no Card, never changes, and stays when the Scholar Archive moves or drops its copy. A Card may show only artifacts ZET holds.
_Avoid_: Attachment, asset, embed, image card

**Literature Card**:
A Card representing one bibliographic work. It carries bibliographic metadata in addition to Card text, and is the root of its own Literature tree.
_Avoid_: Reference Card

**Tree root**:
The Topic or Literature Card at the top of a tree. A Tree root has a direct successor but no side successors, cannot be moved, and deleting it deletes its whole tree.

**Topic tree**:
A Topic and all Regular Cards whose Location starts with `<topic>/`. Also called the idea tree.

**Literature tree**:
A Literature Card and all Regular Cards whose Location starts with `<citation-key>/`. Its Cards can never move to another tree, and Topic tree Cards can never move into it.
_Avoid_: Reference tree

**Citation key**:
The client-assigned, case-significant string address of a Literature Card. It is exactly the entry key in that Card's BibTeX metadata, is never a valid Location, and cannot differ from another Citation key only by case.
_Avoid_: Literature Location, Literature ID

**Workspace**:
One of the two halves of the archive: the **human** Workspace and the **agent** (AI agent) Workspace. Every tree belongs to exactly one Workspace, and every Card in a tree shares its Workspace. Addresses are unique across both Workspaces, so Links may cross them, but Cards and trees can never move between them. Archives created before Workspaces existed hold only human Cards.
_Avoid_: Space, vault, account

**Zone**:
One of the two kinds of tree inside a Workspace: the Topic zone holds its Topic trees, and the Literature zone holds its Literature trees. Each Zone may hold many trees.
_Avoid_: Section, folder

**Access**:
Which Workspaces a client may change. Every client may read both Workspaces. The human's own tools (the Session, the GUI, and shell commands) may change both; the MCP server may change only the agent Workspace, and refuses any change that would rewrite the body of a human Card. Regenerating a human Card's Reverse links is not a change to it.

**MCP server**:
The Model Context Protocol server started with `zt mcp`. It speaks JSON-RPC over stdio, offers every Card operation as a tool, and acts with agent Access.
_Avoid_: API, bot

**Scholar Archive**:
The user's separate local archive of scholars' publications (Works, abstracts, sources, journals, workspaces) and the PDF/Markdown documents linked to them, served at `http://127.0.0.1:8765`. ZET reads it; it is never part of the card archive.
_Avoid_: Paper database, library

**Scholar Archive bridge**:
The authenticated, read-only connection between ZET and the Scholar Archive. Each side has its own connector; only a paired ZET may use it, and every request and answer is signed with the pairing secret. It exchanges information only: it never changes the Scholar Archive, and nothing read through it changes a Card. Taking an artifact through it copies the image into ZET's own `artifacts/` folder.
_Avoid_: API, sync, import

**Location**:
A unique string address for a Topic or Regular Card, such as `1/0`, `1/1`, `1/2|c|4|b|b`, or `Smith2024/1|a`. The text before `/` is the tree id: a Topic number or a Citation key.
_Avoid_: ID, path

**Topic**:
A special Card whose Location is `<topic>/0`. Its title is the topic name, and its body text is a short description.
_Avoid_: Root note, category

**Pointer**:
The current position in the card space for one interactive terminal view.
_Avoid_: Cursor, current note

**Edit caret**:
The text insertion position inside a card being edited.
_Avoid_: Edit cursor, pointer

**Session**:
One interactive terminal view with its own pointer, current Workspace, and command bar.
_Avoid_: Window, shell

**Link**:
A Card-text reference written with the `[[target]]` macro. Its target is a Location or Citation key.
_Avoid_: Edge, relation

**Broken link**:
A link macro whose target card no longer exists after deletion.
_Avoid_: Missing reference, dead edge

**Reverse link**:
Generated text on a target card showing which source cards link to it.
_Avoid_: Backlink, inbound edge

**Direct successor**:
The single continuation card from another regular card. It is recorded with a number in the location.
_Avoid_: Direct success, next note

**Side successor**:
An alternative continuation from another regular card. It is recorded with a letter in the location, and a card may have many side successors.
_Avoid_: Side success, branch

**GUI**:
The local browser interface the service runs while it is up; `zt gui` tells where it listens. It renders Card Markdown (with TeX math in `$...# ZT Note System

ZT is a local note system built around addressable Markdown cards, with terminal navigation and a local GUI.

## Language

**Card**:
A note item in the system. A Card is one Markdown document: a `# Title` heading, a Markdown body, and a generated Reverse link section after the `<!-- zt:reverse-links -->` marker. It has a unique address.
_Avoid_: Note item, note record

**Regular Card**:
A non-root Card in a Topic tree or Literature tree.
_Avoid_: Normal Card

**Card type**:
What a Card holds: `idea` (the default), `event`, or `model`. Only Regular Cards can be Event or Model Cards; Topic and Literature Cards are always Idea Cards. The type stays with the Card when it moves.
_Avoid_: Card kind (kind means Topic, Literature, or Regular), tag, category

**Idea Card**:
A Card of type `idea`: one idea or claim, in the Card's own words. Most Cards are Idea Cards.

**Event Card**:
A Regular Card of type `event`: it records one event that happened in the real world (what, when, where, who, and the source that reports it), not an idea or an argument. Idea Cards that use the event as evidence or illustration link to it.
_Avoid_: Fact card, case

**Model Card**:
A Regular Card of type `model` without a Model facet: the entry point of one whole model. It states the model's question and what the model shows; it may list the model's parts, and a Link from it to a Model part takes that part into the model. Every Card that derives, interprets, extends, or criticises the model links to it or to the part it concerns, so their Reverse links list the discussion of the model.
_Avoid_: Theory card, formula card, model header

**Model part**:
A Regular Card of type `model` with a Model facet. It presents one part of a model in depth and belongs to the Model Card it is filed under (as a successor of the Model Card or of another of its parts), if any, and to every Model Card that links it. So a part may be filed wherever it first connects, and one part may serve several models; a part in no model is reported. A Model Card with its parts is one **model**; which parts it has, what they are called, and how they relate is decided by whoever writes the model. ZT computes the structure from Locations, facets, and Links.
_Avoid_: Sub-card, child model, model section

**Model facet**:
What a Model part presents in its model: a short label (at most 32 characters, one line) chosen by the model's author, such as `设定`, `均衡`, `数值例子`, `a worked example`. There is no required set or order of facets; `setting`, `assumptions`, `timing`, `dynamics`, `solution`, `results`, `intuition`, and `limits` are only stored in one spelling. A model may hold several parts with one facet.
_Avoid_: Role, section, aspect, slot

**Emphasis**:
How much of a reader's attention a Model part asks for: `core` (the heart of the model, what a reader must see first), `aside` (detail a reader may skip), or none (a regular part). Only Model parts have one; the GUI spends ink and space by it.
_Avoid_: Priority, weight, importance

**Model relation**:
A Link from one Model part to another part of the same model. The sentence that holds the Link is the relation's reason, and its words give the relation a Relation kind. A model's relations need not form a sequence or a tree, and they are separate from where the parts are filed.
_Avoid_: Dependency edge, arrow

**Relation kind**:
What a Link says about the Card it names, read from the words its author wrote before it in the same sentence: `rests-on` (`依赖`, `基于`, `代入`, `推导自`, `rests on`, `uses`), `illustrates` (`例示`, `案例`, `for example`), `contrasts` (`对照`, `区别`, `unlike`), `relaxes` (`放宽`, `推广`, `extends`), `corresponds` (`对应`, `类比`, `maps onto`), `objects` (`反驳`, `批评`, `counterexample`), or `refers` when no such word stands before it. A Link right after another in the same sentence shares its kind. Kinds are computed, never stored; only `rests-on` places a part on the Model board.
_Avoid_: Link type, edge label

**Loose part**:
A Model part in no Model relation: it links no other part and no part links it, such as a historical case or a variant set beside the model.
_Avoid_: Orphan, detached part (a detached part is one that belongs to no model)

**Model neighbour**:
A Card outside a model that links one of its Cards or is linked by one, with the Relation kind and sentence of that Link, such as an Idea Card that relaxes an assumption or another model a part corresponds to. Tree roots are not neighbours.
_Avoid_: Discussion card, external part

**HTML chart**:
HTML in a Card body that the GUI shows as a figure: a ```` ```chart ```` fence, or a raw HTML block that opens with a chart element such as `<figure>`, `<div>`, `<svg>`, or `<table>`. It runs sandboxed, apart from the GUI and its token; it may draw with scripts, use TeX, and name Cards with `[[address]]` or `data-zt="<address>"` to let a reader open them. Text inside a chart fence is not a Link.
_Avoid_: Widget, embed, diagram card

**Artifact**:
An image a Card shows: a figure, table, or cover taken from a Scholar Archive Work's Markdown. A Card shows it with `![caption](artifact:<sha256>)`, where the address is the SHA-256 of the image's bytes. ZET keeps the bytes once, in the archive's `artifacts/` folder, beside a record of where they came from; an artifact belongs to no Card, never changes, and stays when the Scholar Archive moves or drops its copy. A Card may show only artifacts ZET holds.
_Avoid_: Attachment, asset, embed, image card

**Literature Card**:
A Card representing one bibliographic work. It carries bibliographic metadata in addition to Card text, and is the root of its own Literature tree.
_Avoid_: Reference Card

**Tree root**:
The Topic or Literature Card at the top of a tree. A Tree root has a direct successor but no side successors, cannot be moved, and deleting it deletes its whole tree.

**Topic tree**:
A Topic and all Regular Cards whose Location starts with `<topic>/`. Also called the idea tree.

**Literature tree**:
A Literature Card and all Regular Cards whose Location starts with `<citation-key>/`. Its Cards can never move to another tree, and Topic tree Cards can never move into it.
_Avoid_: Reference tree

**Citation key**:
The client-assigned, case-significant string address of a Literature Card. It is exactly the entry key in that Card's BibTeX metadata, is never a valid Location, and cannot differ from another Citation key only by case.
_Avoid_: Literature Location, Literature ID

**Workspace**:
One of the two halves of the archive: the **human** Workspace and the **agent** (AI agent) Workspace. Every tree belongs to exactly one Workspace, and every Card in a tree shares its Workspace. Addresses are unique across both Workspaces, so Links may cross them, but Cards and trees can never move between them. Archives created before Workspaces existed hold only human Cards.
_Avoid_: Space, vault, account

**Zone**:
One of the two kinds of tree inside a Workspace: the Topic zone holds its Topic trees, and the Literature zone holds its Literature trees. Each Zone may hold many trees.
_Avoid_: Section, folder

**Access**:
Which Workspaces a client may change. Every client may read both Workspaces. The human's own tools (the Session, the GUI, and shell commands) may change both; the MCP server may change only the agent Workspace, and refuses any change that would rewrite the body of a human Card. Regenerating a human Card's Reverse links is not a change to it.

**MCP server**:
The Model Context Protocol server started with `zt mcp`. It speaks JSON-RPC over stdio, offers every Card operation as a tool, and acts with agent Access.
_Avoid_: API, bot

**Scholar Archive**:
The user's separate local archive of scholars' publications (Works, abstracts, sources, journals, workspaces) and the PDF/Markdown documents linked to them, served at `http://127.0.0.1:8765`. ZET reads it; it is never part of the card archive.
_Avoid_: Paper database, library

**Scholar Archive bridge**:
The authenticated, read-only connection between ZET and the Scholar Archive. Each side has its own connector; only a paired ZET may use it, and every request and answer is signed with the pairing secret. It exchanges information only: it never changes the Scholar Archive, and nothing read through it changes a Card. Taking an artifact through it copies the image into ZET's own `artifacts/` folder.
_Avoid_: API, sync, import

**Location**:
A unique string address for a Topic or Regular Card, such as `1/0`, `1/1`, `1/2|c|4|b|b`, or `Smith2024/1|a`. The text before `/` is the tree id: a Topic number or a Citation key.
_Avoid_: ID, path

**Topic**:
A special Card whose Location is `<topic>/0`. Its title is the topic name, and its body text is a short description.
_Avoid_: Root note, category

**Pointer**:
The current position in the card space for one interactive terminal view.
_Avoid_: Cursor, current note

**Edit caret**:
The text insertion position inside a card being edited.
_Avoid_: Edit cursor, pointer

**Session**:
One interactive terminal view with its own pointer, current Workspace, and command bar.
_Avoid_: Window, shell

**Link**:
A Card-text reference written with the `[[target]]` macro. Its target is a Location or Citation key.
_Avoid_: Edge, relation

**Broken link**:
A link macro whose target card no longer exists after deletion.
_Avoid_: Missing reference, dead edge

**Reverse link**:
Generated text on a target card showing which source cards link to it.
_Avoid_: Backlink, inbound edge

**Direct successor**:
The single continuation card from another regular card. It is recorded with a number in the location.
_Avoid_: Direct success, next note

**Side successor**:
An alternative continuation from another regular card. It is recorded with a letter in the location, and a card may have many side successors.
_Avoid_: Side success, branch

**GUI**:
 and `$...$`, and HTML charts) and changes Cards through the same rules as the Session. An open GUI window is not a Session: it goes down with the service.
_Avoid_: Web app, frontend

**Folgezettel map**:
The GUI view of one tree, laid out by address: Direct successors continue downward and each Side successor starts a new column.
_Avoid_: Tree diagram

**Table**:
The GUI view where a reader lays Cards from anywhere in the slip-box side by side to work with them as one set. A Card on the Table stays filed in its tree. The Card held (the Pointer, when it lies on the Table) is squared up and lifted, the Cards it relates to keep their ink and their threads (each named by its Link kind), and the rest recede; its neighbours not yet on the Table wait within reach beside it.
_Avoid_: Canvas, board, workspace

**Spread**:
One named set of Cards laid out on the Table: which Cards, where each lies, and which lies on top. The archive keeps every Spread; a Spread follows its Cards through moves and renames, loses a Card that is deleted, and changes no Card. Filing a Spread as a card writes a Card that lists its Cards with Links.
_Avoid_: Collection, selection, set

**Model board**:
The GUI view of one model, below a Model Card or Model part: the Model Card as the way in and each part as a small card under the parts it rests on, with a thread for each Model relation (a dashed arc named by its kind when it is not `rests-on`) and Loose parts in a tray. It is laid out by `rests-on` relations, not by address, and inked by attention: the open part, the parts it relates to (with their kinds and reasons), and core parts stay legible; the rest recede to their facet. Below it lie the open Card's Model neighbours and the other models a shared part serves.
_Avoid_: Model map, model diagram

**Model range**:
The Model Card and the Model parts filed under it.
_Avoid_: Model scope, model folder

**Model member**:
A Card of a model: a Card in its Model range, or a shared part that the Model Card takes in by a Link.
_Avoid_: Model element

**Build order**:
The order in which a reader builds a model. The **main line** starts at the Model Card's first Side successor (the setting) and follows its Direct successors in the range: the setting, then each step, then each result. A part below a main-line part through a Side successor holds an example, a case, or a variant of that step. A later Side successor of the Model Card stands outside the build (a correspondence, a limit, a history). A shared part takes its place just before the first main-line step that rests on it. ZT computes the build order (`get_model` `build.order`); the divider tabs follow it.
_Avoid_: Reading order, sequence

**Derivation part**:
A Model part whose only job is one step of a derivation: it starts from parts earlier in the build order and ends at an intermediate result that a later part uses. Its facet starts with `推导`, and its emphasis is normally `aside`. The Model board draws it as a slim slip on the thread between the parts it connects.
_Avoid_: Proof card, helper card, bridge card

**Self-sufficient model**:
A model a reader can build from nothing by reading only its own Cards in build order. Every rests-on relation from a Card in its range targets a Model member, and no main-line part rests on a later part. ZT reports the faults (`closure`, `forward`, `unanchored`) in the build report; it does not enforce them.
_Avoid_: Closed model, complete model

**Elaboration level**:
How fully a model is built out, stored on its Model Card (never on a part): level 1, `concept`, or level 2, `formal`. A Model Card without a level has not been classified, and its build report applies the formal rules. The book digest decides the level from how the source states the model, and builds each model only as far as its level asks.
_Avoid_: Depth, detail level, maturity, tier

**Concept model**:
A model of Elaboration level 1. The source states it in words or a diagram: a mechanism, a typology, a framework, or a verbal argument, or a formal model the source only cites. Its Model Card and at most a few parts state the question, the mechanism, and the result in words; it has no derivation ledger and no Derivation parts, and the build report lists any Derivation part in it (`beyond_level`).
_Avoid_: Informal model, sketch, light model

**Formal model**:
A model of Elaboration level 2. The source states it in mathematics: symbols, equations, a derivation, or a proposition with a proof. The book digest builds it as a Self-sufficient model: the main line in build order, a setting with its symbol and term tables, and a Derivation part for each step the source skips.
_Avoid_: Math model, full model, rigorous model

**Tolerated premise**:
A premise of a shared part that lies in the shared part's home model but is not a member of this model. The build report lists it; it is not a fault.
_Avoid_: Borrowed premise, leak

**Card language**:
The plain-text rules Card titles and bodies follow: ASD-STE100 (Simplified Technical English), adapted for Chinese: one statement per sentence, short sentences, no semicolons, active voice, a verb for an action, one term per meaning (a model's term table), and no hedges or praise words. Formulas, Links, charts, and quotations are exempt.
_Avoid_: Style guide, tone

**Insert**:
Putting a new Card in the place of an existing Regular Card: that Card and its subtree move down one place and it becomes the new Card's Direct successor (`X/2` becomes `X/3`; before `P|a`, the old `P|a` becomes `P|a|1`). A move may also take another Card's place (`before`), and may move a Card **alone**, without its successors, while its Direct successors move up to close the gap. Each renumbers Cards, shows every renamed address first, and rewrites every Link, so no reference breaks.
_Avoid_: Splice, shift
