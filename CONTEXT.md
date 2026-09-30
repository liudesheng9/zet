# ZT Note System

ZT is a local note system built around addressable Markdown cards, with terminal navigation and a local GUI.

## Language

**Card**:
A note item in the system. A Card is one Markdown document: a `# Title` heading, a Markdown body, and a generated Reverse link section after the `<!-- zt:reverse-links -->` marker. It has a unique address.
_Avoid_: Note item, note record

**Regular Card**:
A non-root Card in a Topic tree or Literature tree.
_Avoid_: Normal Card

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
One interactive terminal view with its own pointer and command bar.
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
The local browser interface started with `zt gui`. It renders Card Markdown and changes Cards through the same rules as the Session; it counts as a Session while open.
_Avoid_: Web app, frontend

**Folgezettel map**:
The GUI view of one tree, laid out by address: Direct successors continue downward and each Side successor starts a new column.
_Avoid_: Tree diagram
