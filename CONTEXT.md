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
A Regular Card of type `model` without a Model facet: it presents one whole model (its question and an overview) and lists the model's Model parts with Links. Every Card that derives, interprets, extends, or criticises the model links to it or to the part it concerns, so their Reverse links list the discussion of the model.
_Avoid_: Theory card, formula card

**Model part**:
A Regular Card of type `model` with a Model facet, filed under its Model Card: as a successor of the Model Card or of another of its parts. It presents one part of the model in depth, links its Model Card, and links the other parts it rests on. A Model Card with its parts is one **model**; which parts it has, what they are called, and how they relate is decided by whoever writes the model. ZT computes the structure from Locations, facets, and Links.
_Avoid_: Sub-card, child model, model section

**Model facet**:
What a Model part presents: a short label (at most 32 characters, one line) chosen by the model's author, such as `设定`, `均衡`, `a worked example`. The common facets `setting` (agents, assets, actions, payoffs), `assumptions`, `timing` (order of moves and information), `dynamics` (how the state evolves: laws of motion, best responses, feedback), `solution` (equilibrium or optimum), `results`, `intuition`, and `limits` are a suggested vocabulary, not a required set or order. A model may hold several parts with one facet.
_Avoid_: Role, section, aspect

**Emphasis**:
How much of a reader's attention a Model part asks for: `core` (the heart of the model, what a reader must see first), `aside` (detail a reader may skip), or none (a regular part). Only Model parts have one; the GUI spends ink and space by it.
_Avoid_: Priority, weight, importance

**Model relation**:
A Link from one Model part to another part of the same model: the linking part rests on the linked one. The sentence that holds the Link is the relation's reason. A model's relations need not form a sequence or a tree, and they are separate from where the parts are filed.
_Avoid_: Dependency edge, arrow

**Loose part**:
A Model part in no Model relation: it links no other part and no part links it, such as a historical case or a variant set beside the model. It still links its Model Card.
_Avoid_: Orphan, detached part (a detached part is one not filed under a Model Card)

**HTML chart**:
HTML in a Card body that the GUI shows as a figure: a ```` ```chart ```` fence, or a raw HTML block that opens with a chart element such as `<figure>`, `<div>`, `<svg>`, or `<table>`. It runs sandboxed, apart from the GUI and its token; it may draw with scripts, use TeX, and name Cards with `[[address]]` or `data-zt="<address>"` to let a reader open them. Text inside a chart fence is not a Link.
_Avoid_: Widget, embed, diagram card

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
The authenticated, read-only connection between ZET and the Scholar Archive. Each side has its own connector; only a paired ZET may use it, and every request and answer is signed with the pairing secret. It exchanges information only: it never changes the Scholar Archive, and nothing read through it changes a Card.
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
The local browser interface started with `zt gui`. It renders Card Markdown (with TeX math in `$...$` and `$$...$$`, and HTML charts) and changes Cards through the same rules as the Session; it counts as a Session while open.
_Avoid_: Web app, frontend

**Folgezettel map**:
The GUI view of one tree, laid out by address: Direct successors continue downward and each Side successor starts a new column.
_Avoid_: Tree diagram

**Model board**:
The GUI view of one model, below a Model Card or Model part: the Model Card as a header and each part as a small card under the parts it rests on, with a thread for each Model relation and Loose parts in a tray. It is laid out by Model relations, not by address, and inked by attention: the open part, the parts it relates to (with their reasons), and core parts stay legible; the rest recede to their facet.
_Avoid_: Model map, model diagram
