# ZT Note System

ZT is a local CLI note system built around addressable cards and terminal navigation.

## Language

**Card**:
A note item in the system. A Card has a title, body text, Reverse link section, and unique address.
_Avoid_: Note item, note record

**Regular Card**:
A Card in a Topic's successor tree that is neither a Topic nor a Literature Card.
_Avoid_: Normal Card

**Literature Card**:
A Card representing one bibliographic work. It carries bibliographic metadata in addition to Card text.
_Avoid_: Reference Card

**Citation key**:
The client-assigned, case-significant string address of a Literature Card. It is exactly the entry key in that Card's BibTeX metadata, is never a valid Location, and cannot differ from another Citation key only by case.
_Avoid_: Literature Location, Literature ID

**Location**:
A unique string address for a Topic or Regular Card, such as `1/0`, `1/1`, or `1/2|c|4|b|b`.
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
