# The Table and Spreads Design

Status: implemented in zt 0.11.0.

## Source Authority

- `CONTEXT.md` defines **Table** and **Spread**. This document adds a GUI view and one archive record; it changes no Card rule.
- `docs/markdown-cards-and-gui-design.md` remains authoritative for the Map, the Constellation, the Reader, and the writing desk.

## Why

The Map shows a tree by address and the Constellation shows the whole archive, but neither lets a reader pull Cards from different places together and look at them as one set. In Luhmann's practice that is the point of a slip-box: the Cards for a question come out of the box and lie side by side on the table. Three principles govern the design.

1. **Building a set of related Cards comes first.** Gathering Cards that belong together, wherever each is filed, must be quick, and the gathered set must take the most emphasized place on the screen.
2. **Attention is limited.** The view spends emphasis on what the reader is working with and takes it from everything else, by lift, size, and ink, never by blur.
3. **Every component maps to something on a real desk, and moves like it.** Cards lie at a slight angle where they were put down, are picked up and carried, settle when laid down, and go back into the box.

## The Table

The Table is the third stage view, between the Map and the Constellation (`v` cycles them). While it is open, the cabinet recedes into a drawer (the menu button, `/`, or search opens it), so the Table gets the width.

### Laying Cards on the Table

- `p` lays the Card being read on the Table, or puts it back in the box. The Reader has the same button.
- `P` lays the Card down with every Card it relates to (its Links, the Cards that link it, its parent, and its successors), gathered around it.
- Shift-click a card in the Map, or `+` on a search hit, lays it down without leaving where you are.
- The Card being read but not on the Table rides **in hand** at the bottom edge; clicking it lays it down.
- **Within reach**: the Card held shows its neighbours that are not yet on the Table as slips tucked beside it: kinded Links first (rests on, illustrates, contrasts with, …), then plain Links, then its tree neighbours. One click lays a slip down beside the Card held; **Lay all** lays them all and gathers them around it. The slips sit in the nearest free gap so they never cover a Card.
- A new Card takes the nearest free place beside the Card held (right before left, the same row before another), so a set grows outward from where the reader works.

### Attention

- The Card held (the Pointer, when it lies on the Table) is squared up, scaled up, ringed in the Pointer colour, and lies on top.
- The Cards it relates to keep full ink, a blue mark on their edge, and their threads.
- The rest lie flatter and paler, still legible, and come back on hover.
- Threads are drawn only between Cards on the Table. The held Card's threads are inked and named by their kind; a hovered Card's are half-inked; the rest are faint. Each kind has its own stroke (solid rests on, dotted illustrates, dashed contrasts, red objects).
- The surface is lit in the middle and darker at the edges, like a desk under a lamp.
- Framing keeps cards legible: an automatic fit never goes below a readable scale; when the set does not fit, it frames the Card held. Only the Fit button shows the whole set at any scale.

### Physical behaviour

- Each Card rests at a small fixed angle derived from its address, so the same set always looks the same.
- Picked up, a Card lifts (larger shadow, slightly larger) and swings with the hand's horizontal speed.
- Laid down, it settles with a little overshoot, and the Card laid down last lies on top.
- A Card laid down from within reach slides out from its slip; one laid down from elsewhere is dealt onto the Table from above.
- While a Card is carried, the box slides up at the lower left. Dropping the Card on it, or pressing its corner button, puts it back: it falls into the box and leaves the Table.
- In the Map, a Card that lies on the Table carries an out-card flag on its top edge, as a library marks a card taken from the drawer.

### Arrangements

Arrangements only suggest places; the reader moves Cards freely afterwards, and the arrangement is saved like any move.

- **Gather**: the Card held in the middle and the others in the cells of a grid around it, nearest cells first. The Cards it relates to take the cells beside it. The Card held stays where it lies.
- **Line up**: rows in reading order. Each Card that no other Card on the Table links starts a chain, a chain follows Links depth first, and Folgezettel order breaks ties.
- **By tree**: one column per tree in Folgezettel order, shingled the way cards stand in a drawer, each covering the one before it below its title.

All three are deterministic and run in time at most quadratic in the Cards on the Table (`src/gui/layout.js`).

## Spreads

A Spread is one named set of Cards on the Table. The reader may keep many and switch between them from the Spread button.

- Stored in the archive's `metadata` table under `gui_spreads` as `{ "current": <id>, "spreads": [{ "id", "name", "cards": [{ "at", "x", "y" }] }] }`. Card order is stacking order: the last lies on top. A Spread survives restarts and a change of GUI port.
- The GUI reads them with `GET /api/spreads` and replaces them with `POST /api/spreads`, saved a moment after each change. The server checks the shape and bounds (at most 64 Spreads of 400 Cards, names of 80 characters, finite coordinates); a Card listed twice keeps its last place.
- **A Spread follows its Cards.** Every change of addresses (a move, a side-successor compaction after a delete, a Citation-key rename) rewrites the addresses on every Spread in the same transaction, and a deleted Card leaves every Spread. Before any Card change the GUI saves its pending Spread first, and it reads the Spreads back after.
- A Spread changes no Card and is not a Link. **File as card** writes it into the slip-box: it opens the writing desk for a new Card after the Card held (a side successor of a Regular Card, else a direct successor), titled with the Spread's name and listing its Cards as `- [[address]] title` in reading order, and lays the new Card on the Table.

## Link kinds in Card summaries

Every Card summary (`/api/graph`) now carries `kinds`: for each Link whose words announce a kind, its target and that kind (`rests-on`, `illustrates`, `contrasts`, `relaxes`, `corresponds`, `objects`). Plain references are left out. The Table uses them to name threads and order what lies within reach.

## Out of scope

- Spreads are not exposed over MCP yet.
- Spreads are not part of `zt dump`.
- Several windows do not merge concurrent changes to the same Spreads; the last save wins.
