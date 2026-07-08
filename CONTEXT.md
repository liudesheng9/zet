# ZT Note System

ZT is a local CLI note system built around addressable cards and terminal navigation.

## Language

**Card**:
A note item in the system. A card has a title, body text, reverse-link section, and unique location.
_Avoid_: Note item, note record

**Location**:
A unique string address for a card, such as `1/0`, `1/1`, or `1/2|c|4|b|b`.
_Avoid_: ID, path

**Topic**:
A special card whose location is `<topic>/0`. Its title is the topic name, and its body text is a short description without outbound links.
_Avoid_: Root note, category

**Pointer**:
The current position in the card space for one interactive terminal view.
_Avoid_: Cursor, current note

**Session**:
One interactive terminal view with its own pointer and command bar.
_Avoid_: Window, shell

**Link**:
A card-text reference written with the `[[location]]` macro.
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
