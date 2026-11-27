# Free Connection Trees: free composition, a root without meaning, many trees, the projection, and tree actions

Owner: Xingtong Chen
Status: implemented in zt 0.70.0 (2026-11-26); reviewed twice by Astra (§6); development record in §7
Builds on: docs/multi-tree-coordinates-proposal.md (zt 0.65.0–0.69.1), block doc docs/wiki/coordinates.md

## 1. The owner's request

The owner asked for these changes to Connection trees (关联树):

1. **Free composition.** A Connection tree connects any Cards and any set of Cards, in the arrangement the author wants. The arrangement does not have to follow the home tree a Card comes from.
2. **A root without meaning.** The root of a Connection tree has no special meaning. A Literature root carries the source's metadata and a Topic root names a Topic; a Connection root is only a Card. The author can write in it or leave it empty, and both are acceptable.
3. **Many trees.** There will be more Connection trees than Literature trees. Creating one must be cheap and every view must stay usable with many.
4. **The projection.** When the owner reads a Literature tree (or a Topic tree), the GUI shows the Connection trees that cross it. A Connection tree is in a higher dimension; the GUI shows only the part of it that lies in the plane of the tree being read, and the places where it crosses that plane.
5. **Tree actions.** Everything the GUI can do with a Literature or Topic tree it can also do with a Connection tree: open, export, lock (and the Lock panel, note and unlock), and delete.
6. **Colour.** The basic colour of a Connection tree is the colour of a Literature tree, not plum.

## 2. Where we are (zt 0.69.1)

- Connection trees exist (`~n`): stations mount any Card by identity; native `idea`/`event`/`finding` Cards (`~n#k`); tree connections between stations; Locks, pins, drift, export (portable version 3), Session verbs and the MCP tools (docs/wiki/coordinates.md).
- **Composition is free in the data, narrow in the GUI.** Any Card can be mounted anywhere in any order, but the GUI mounts one Card per prompt. Seeding from a selection exists on the server (`seed_connection_tree` with `from: "search"`) and the GUI offers only the Dependency Lens seed.
- **The root has a meaning.** `create_connection_tree` refuses an empty title ("a Connection tree needs a title"), the root's title is the tree's name in every list, and the GUI asks for a title first.
- **Few-tree assumptions.** The cabinet lists Connection trees in a side section; the desk shows only Topic and Literature stacks; hues come from a palette of ten.
- **No projection.** A Literature or Topic tree shows only stamps (`elsewhere`, `@N`) for the coordinates its Cards have in other trees. Nothing shows how a Connection tree runs through the tree being read.
- **Few tree actions.** A Connection drawer opens the tree and nothing else: no tree menu, no export, no delete, no Lock panel entry. Lock is a button on the Map bar. The Lock panel names a Connection root as a Literature tree.
- **Colour.** A Connection tree's own Cards, its stamps and its drawer address are plum (`OWN_HUE = 328`).

## 3. Decisions

### D1. Free composition

- **Any set of Cards at once.** `place_card` takes `targets` (a list of addresses, Citation keys or handles) besides `target_at`. With `edge: "direct"` the Cards form a Direct chain after the parent, in the given order; with `edge: "side"` each becomes the next Side branch of the parent. The batch is resolved against the tree as it stands and deduplicated by Card identity in first-occurrence order (a promoted handle and its Topic address name one Card). One transaction, one revision, one gloss review (only the station they displace; the plan names the order and the displaced station). A Card already in the tree is skipped and named (`already_placed`); a batch that places nothing is a successful no-op that keeps its `op_key` receipt; a bad target refuses the whole batch. `gloss` is refused with `targets` (a gloss belongs to one station) and so is `edge: attach` (a Topic-side mount takes one Card). At most 500 Cards a call.
- **Seed from the Table.** The GUI seeds a new tree from the Cards on the Table (`seed_connection_tree`, `from: "search"`, the Cards in reading order of the Table, top to bottom, then left to right) and from any Card ("New Connection tree from this Card": an untitled tree with that Card at `~n/1`).
- **The GUI Mount accepts several Cards** (addresses separated by spaces or commas) and sends one `place_card` with `targets`.
- **Kept on purpose** (the invariants of docs/multi-tree-coordinates-proposal.md §3.4): one coordinate per Card per tree (a Card in two roles is one mount plus native Cards or connections), the root's Direct chain as the top-level peers (no Side successor of the root), single-Card mounts (no subtree explosion), and the Phase 5 gate on native structured types. None of them limits which Cards a tree holds or how they are arranged; each protects addressing that other features rely on.

### D2. The root has no meaning

- `create_connection_tree` takes an optional `title`. Without one, the root Card is stored with its handle as its title (`# ~7`): every Card keeps a non-empty title (`card::parse` refuses an empty one, and 100 readers rely on it), and a title equal to the Card's own handle carries no meaning.
- **Untitled reads as untitled.** A root whose title is its own handle is *untitled*: `tree_summary` answers `title: ""` and `untitled: true`; the GUI shows the address alone (drawer `(untitled)`, the Map root with no title line, the reader's title faint). Writing a title or a body later is an ordinary Card edit. Clearing the title (an edit to an empty title) is refused like for any Card; the GUI offers "Clear title", which writes the handle.
- **The title is not the tree's identity.** Lists sort and filter by the root's title when there is one and by the handle always; nothing keys on the title. `seed_connection_tree` also takes an optional `title`.
- The GUI's "New Connection tree" creates the tree at once (no prompt) and opens its root.

### D3. Many trees

- Creating a tree is one action (D2). `list_connection_trees` gets `sort` (`number`, the default for every existing caller, or `recent`, newest first, which the GUI asks for; handles are never reused, so the highest number is the newest) and filters on the handle, the title and the purpose before it counts anything.
- The desk gets a **Connection zone** of stacks after Literature, the same stacks and tree menu as the other zones; the cabinet section keeps its filter.
- The projection (D4) reads only the Connection trees that touch the tree being read, by index, so its cost grows with the crossings, not with the number of trees.
- The colour (D6) no longer spends palette hues on Connection trees, so many trees never exhaust the palette; trees are told apart by their handle stamp, which each line and stamp carries.

### D4. The projection: the Connection trees that cross the tree being read

Picture each Topic or Literature tree as a plane and each Connection tree as a structure in a higher dimension that passes through some Cards of the plane. The projection of a Connection tree `~n` onto the plane of tree `T` is:

- its **stations in the plane**: the stations of `~n` whose live Card has its home in `T`;
- its **structure lines in the plane**: for each in-plane station, a line to its nearest in-plane ancestor in `~n` (its structural parent, or the first in-plane station above it). `hops` counts the stations outside the plane in between (0: the two are adjacent in `~n`). The edge kind of the first step (`direct`, `side`) is kept;
- its **connection lines in the plane**: every tree connection of `~n` whose two stations are both in the plane, with its direction, label, kind and `connection_id`;
- its **exits**: for each in-plane station, how many of its structural neighbours (parent, children) and tree connections lie outside the plane. An exit is where `~n` leaves `T`.
- **The root** is the tree itself: it is in no plane, it ends the climb to an ancestor, and it never counts as an exit, neither by structure nor by a connection to it. Membership follows the live canonical home: a promoted native Card is in its Topic's plane; a Missing mount is in no plane but still counts as a hop and an exit.

Server. `connection::crossings(conn, tree)` answers `{"<~n>": {tree, title, untitled, stations: [{address, placement_id, coord}], lines: [...], exits: {address: n}}}` for every Connection tree with at least one station in `T`. It reads the in-plane stations with the same index range as `connection::elsewhere`, then, per touched tree, only that tree's `placements` (by `host_tree_key`) and `placement_edges` (by `tree_id`). `GET /api/tree` adds it as `crossings`, read with `elsewhere` from one membership read and one shape per Connection tree (`connection::tree_coordinates`), in one read transaction at the revision of the Snapshot that supplies the Cards (a write between the two makes the answer use a fresh Snapshot). The page reads the open tree again whenever the archive revision moved, because a coordinate-only write changes no Card summary. A missing (retired) Card is never in the plane. Topic-side mounts are not part of the projection (they already show as tabs beside their Topic Card).

GUI (the attention rules of the Map: draw for the focus and the hand, recede the rest):

- By default the Map draws the projection lines that touch the focused Card and the hovered Card, from every crossing tree, like its Link arcs.
- A **Crossings** switch beside the Term switch draws every projection line of every crossing tree in view, as the whole-plane picture. Its state is kept like the Term switch.
- A structure line is a thin solid thread in the Connection ink (the Literature colour), bowed out to the right 40 px wider than a Link arc so the two never lie on one line; a line with `hops > 0` is dotted and its title says "through N stations outside this tree"; a connection line is dashed and carries its label. Each line's title names its tree (`~n · title`) and what the line says.
- An in-plane Card with exits shows a small notch `~n ↗` on the lifted card only (the stamps already list its places; the notch says the tree continues outside).
- The lifted card's stamps keep switching trees (↗), as in 0.69.0.

### D5. Tree actions for Connection trees

- One **tree menu** for every role: the cabinet drawer and the desk stack of a Connection tree open it by right-click, a long press, Shift+F10 or the menu key, as for Topic and Literature trees. Rows: Open tree, Export tree, Lock tree… (or Lock panel, Lock note, Unlock tree…), Delete tree….
- **Delete tree…** is in the menu of every role. It asks the same plan as deleting the root Card (`POST /api/delete` for the menu's own root, never the open Card; the plan's lines and confirmation word, the coordinate impact lines). The confirmation is bound to the plan it showed: the plan carries `plan_hash` and a confirmation with a hash that no longer matches is refused as stale, so a tree that grew after the preview is never deleted on the old word. For a Connection tree the plan names its stations, connections and native Cards; mounted Cards stay at home. A locked tree refuses, as now.
- At a Connection root the reader already carries the root's lock line like a Topic or Literature root (Lock tree… / Lock panel / Unlock tree…). The Map bar's lone Lock button becomes `Tree ▾`, which opens the same tree menu. The Lock plan also lists the mounted Cards the Lock pins (`coordinates.pins`).
- The Lock panel names a Connection root as a Connection tree (today it says Literature).
- Export, Lock and Delete keep their server paths: `POST /api/export` (portable version 3), `POST /api/lock` and friends, `POST /api/delete`. No new write route.

### D6. Colour

- A Connection tree's own colour is the Literature colour: the Literature ink (`--lit`) on its drawer's edge and stack, the Literature root stock (`--lit-soft`) on its root card, and the Literature hue (`OWN_HUE`, the OKLCH hue of `--lit`) on its stamps, chips, lines and native Cards.
- The palette for mounted home trees drops the two hues next to it (red 18 and orange 44) and takes plum (328) back, so no mounted tree is mistaken for the Connection tree's own Cards.

## 4. How the change acts with the other features

- **Card rules and the write gate.** The untitled root is an ordinary `idea` Card titled with its handle; the gate checks it as the Connection kind does today. No Card rule changes.
- **Links.** `[[~7]]` still names the root; a Link label falls back to the handle when the root is untitled.
- **Locks.** A locked Connection tree refuses `place_card` with `targets` like any placement; a locked source Card can still be mounted. The projection only reads. Delete tree on a locked tree is refused by `tree_lock::ensure_unlocked`, and a tree whose Cards another Lock pins is refused by the pins.
- **Write scope.** A Literature-scoped MCP connection may read Connection trees and runs no Connection-tree write; `targets` changes nothing there.
- **Export.** Export of `~n` keeps version 3; an untitled root exports with its handle as title. A Literature or Topic export does not include the projection (it is a view, not data).
- **Session and MCP.** The Session's station verbs and the MCP tools keep their names; `place_card` gains `targets`, `create_connection_tree`/`seed_connection_tree` make `title` optional, `list_connection_trees` gains `sort` and `untitled`.
- **Performance.** `GET /api/tree` is cached per Snapshot; `crossings` adds per-tree indexed reads only for trees that cross `T`.

## 5. Plan and tests

1. Server: optional title, `untitled`, `sort`, `targets`, `crossings` (+ `/api/tree`); tests in `tests/connection_free.rs`.
2. GUI: tree menu on Connection drawers and stacks, Connection zone, Delete tree, root lock line, Lock panel kind, mount several, seed from the Table and from a Card, instant new tree; the projection and the Crossings switch; colour. Unit tests of `connections.js` rules under Node in `src/gui.rs`.
3. Docs: this proposal's record (§7), block docs `coordinates.md` and `gui.md`, MCP tool descriptions, CONTEXT.md vocabulary (Projection, Crossing).
4. Release: zt 0.70.0, push to main, deploy on the live service.

## 6. Review log (Astra)

**Round 1 (design and audit, before the implementation).** Astra accepted D1–D6 with changes and audited the zt 0.69.1 Connection-tree code. Design changes taken: resolve and splice a `targets` batch once and keep the receipt of an all-skipped batch (a no-op answered `applied: false` would roll back and look like a plan); keep the handle-as-title representation but derive `untitled` only for a Connection root and only in the views, and never turn an unreadable root into an untitled one; `recent` means newest created, by handle; one root convention for the projection (the root counts nowhere); share one membership read and one shape per host between `elsewhere` and `crossings`; read them at the Snapshot's revision; Delete tree… for all three roles with the menu's explicit root; use `--lit`/`--lit-soft` directly. Defects found and fixed in this release:

| Severity | Defect | Fix |
|---|---|---|
| Blocker | A delete confirmation could apply a larger delete than its preview (a native Card added between preview and confirm). | `ops::delete_reviewed`: the plan carries `plan_hash`; a passed hash that no longer matches is refused (GUI Delete Card, Delete tree, MCP `delete_card`). |
| Important | A schema-closed MCP client could not confirm a gloss-reviewing `place_card` (no `expected_revision`/`plan_hash` in the schema). | Both added to the tool schema. |
| Important | Home Maps and reader stamps went stale after coordinate-only writes; `/api/tree` mixed revisions and swallowed coordinate errors. | `refresh` re-reads the open tree when the revision moved; `tree_raw` reads at the Snapshot's revision and reports errors. |
| Important | A station beyond the first 5,000 of a tree could not open. | `loadStation` reads every page. |
| Important | A Topic-side coordinate could mark the Card's Connection station instead of the mount. | The mount is found by its coordinate. |
| Important | Per-tree work multiplied: `elsewhere` rebuilt a host's shape per placement, the list counted before filtering, the overview scanned all Cards per root. | One shape per host, filter before counts, one pass in `Snapshot::trees`. |
| Important | The Lock dialog did not show the mounted Cards a Connection-tree Lock pins. | `Pinned mounts` fold and count. |
| Important | Portable v3 showed a mounted Card in the host's Workspace. | The export writes the Card's own `workspace`; the offline rules read it. |
| Minor | A promoted handle's Link lost its live title. | `link_title` reads the Card at its address today. |
| Minor | The Lock card named a `~n` root a Topic or Literature tree; the Map offered Lock… on a locked tree; the cabinet filter missed handles. | `lockKind`; the `Tree ▾` menu; the filter reads the handle. |

**Round 2 (the implementation).** Astra confirmed the block splice (Direct and Side), the revision-consistent tree read, and that the Session station verbs and the MCP Literature write scope are unchanged. It found:

| Severity | Defect | Fix |
|---|---|---|
| Blocker | An old delete confirmation could still remove a tree connection written after the preview: the hash covered the deleted addresses and the plan lines, and "and their connections" does not change when one is added. | The hash also covers `coord::delete_state`: the text of every deleted Card and every station (slot, gloss) and connection (with its last revision) the delete removes or leaves missing. |
| Important | The first coordinate-only change after boot could leave the Map stale (the revision was unset at boot), and a failed re-read was acknowledged. | The revision is set at boot and acknowledged only after the open tree was read. |
| Important | A station removed elsewhere stayed on screen as an interactive Map; a stale cursor during a paged read failed. | The page leaves the removed station (to its tree's root, else the cabinet); a stale cursor restarts the read once. |
| Important | An all-skipped batch returned success before checking a confirmation of an older plan. | A submitted confirmation is checked first and refused as stale. |
| Important | `extract_branch` still required a title its schema made optional. | Optional title, untitled copy. |
| Important | The portable viewer still gave a mounted Card the host's Workspace in its graph, search and Link answers, and showed an untitled root by its handle twice. | Every portable answer reads the Card's own Workspace; the portable tree and Link HTML read untitled roots like the live page. |
| Important | Parallel projection lines (two assertions over one pair, two trees over one pair) lay on one path. | `CONN.crossingIndex` gives each line of a pair its own lane; lanes lie 14 px apart. The index is built once per tree answer instead of every frame. |
| Minor | Delete tree… stayed clickable on a locked tree. | The row is disabled with its reason. |
| Interaction | "New Connection tree from this Card" was unreachable for Cards read at a station; "Clear title" was missing; the Lock preview counted a target twice when the tree both cites and mounts it. | Both actions are in the station's More menu; Clear title is a base-checked edit back to `# ~n`; the preview counts distinct targets. |

Accepted limits after round 2: the unlock and note views still count only Link pins (the coordinate pins are enforced and listed in the Lock panel's history, not in that count); a Connection tree whose stored shape is damaged is left out of the stamps and the projection without a mark (`zt status` reports it); the catalog read on every refresh costs a few indexed queries per tree, which is small for hundreds of trees and has no batch form yet.

Accepted limit from round 1: a root whose author deliberately writes its own handle as its title reads as untitled; an explicit stored bit would distinguish the two and is not needed now. Search still scores an untitled root's title like its address.

## 7. Development record

Worked in the git worktree `E:\zet-conn` (branch `conn-free` from `390da3cb`) because another session was editing `E:\zet`; built into `D:\zt-target` with `-j 1` and checked free commit memory before every build.

| Step | What changed | Where |
|---|---|---|
| D2 root without meaning | optional title; `root_title` and `untitled`; "(untitled)" in drawer, stack, Map root, reader, Lock card, Link chips and Link HTML | `src/connection.rs` `create_tree_tx`, `create_tree`, `root_title`, `tree_summary`; `src/dependencies.rs` `seed`; `src/gui.rs` `link_title`; `src/gui/app.js` `untitledRoot`, `newConnectionTree`; `src/gui/connections.js` `mapTree`, `treeLabel` |
| D1 free composition | `place_card` `targets` as one block; Mount of several Cards; New Connection tree from a Card and from the Table | `src/connection.rs` `place_many`, `attach`; `src/mcp.rs`; `src/gui/app.js` `mountHere`, `treeFromCard`, `treeFromTable`; `CONN.mountTargets` |
| D3 many trees | `sort`, handle filter before counts; desk Connection zone; one-pass tree counts | `src/connection.rs` `list_trees`; `src/ops.rs` `Snapshot::trees`; `src/gui/app.js` `connectionZone`, `loadConnTrees` |
| D4 projection | `plane_placements`, `host_shapes`, `elsewhere_of`, `crossings_of`, `tree_coordinates`; `/api/tree` `crossings` at the Snapshot's revision; `e-cross` lines, Crossings switch, exits; open tree re-read on a revision move | `src/connection.rs`; `src/gui.rs` `tree_raw`; `src/gui/app.js` `paintEdges`, `setCrossEdges`, `exitMarks`, `refresh`; `CONN.projectionLines`, `visibleCrossings`, `crossingText`, `exitsAt`; `src/gui/index.html` `#cross-toggle` |
| D5 tree actions | one tree menu for Connection drawers, stacks and the Map's `Tree ▾`; Delete tree… for every role; plan-bound deletes; Lock kinds and Pinned mounts | `src/gui/app.js` `treeMenu`, `connMenuTree`, `deleteTree`, `lockKind`, `lockPlanView`; `src/ops.rs` `delete_reviewed`, `delete_plan_hash`, `Outcome::Confirm`; `src/mcp.rs` `delete_card` |
| D6 colour | `OWN_HUE` 36, palette without red 18 and orange 44, plum 328 back; `--lit`/`--lit-soft` on drawer, stack, root, native stations, connection stamps, lines | `src/gui/connections.js`, `src/gui/app.css` |
| Round-1 fixes | see §6 | `src/export.rs`, `src/gui/portable-rules.js`, `src/gui/app.js` `route`, `loadStation` |

Tests: `tests/connection_free.rs` (seven cases: untitled roots, sort and filter and the untitled Link label; block mounts with gloss review, Side order, receipt replay, rollback and refusals; the projection with hops, Side lines, connection lines, exits, the root and a Missing mount; the plan-bound tree delete; one-pass tree counts; export Workspaces; old confirmations refused after a new connection, an all-skipped batch and an untitled extract) and the unit test `gui::tests::connection_trees_wear_the_literature_colour_and_project_onto_the_tree_read`; the Lock-dialog pin test now counts the delete plan hashes. The existing Connection, coordinate and GUI suites pass unchanged.

GUI check on a copy of the live archive (`D:\zt-gui-conn`, port 4799): the desk's Connection zone, the tree menu on a stack and on the Map, an untitled tree created in one click, three Cards mounted in one prompt, Delete tree… with its plan and word, the Lock dialog naming a Connection tree, and the projection of `~1` drawn on `velic2025inflation` with its exit tab. A debug build rebuilds the Snapshot of this 62,000-Card copy in about 17 s after each write, so GUI writes there answer slowly; cached reads answer in a few milliseconds.

**Release (2026-11-27).** Pushed to `main` as `3e2dffbd` (fast-forward from `390da3cb`). Full gate: `cargo test --all-targets --no-fail-fast`, 1,107 tests passed; the one failure, `release_manifest::the_installed_frozen_workflows_are_the_manifest_builds` (the committed `zt-econ-digest.js` does not match its lock), fails the same way at `390da3cb` and is outside this change. Deployed on the live service: backup `D:\zt\backups\zt-before-0.70.0-2026-11-27.sqlite3` (integrity ok, 62,432 Cards), the old binary kept as `zt.exe.0.69.1-old`, `zt down`/`zt up`, `zt status` reports daemon 0.70.0 with coordinates and Locks intact. On the live archive `/api/tree?id=velic2025inflation` answers with the projection of `~1` (three stations, two lines) in 29 ms and the overview in 222 ms.

**Follow-up 0.70.1 (2026-11-27, owner).** On the live service every Card written in a Connection tree wore the root's Literature tint and a red top edge, so the tree read as a pile of root cards. A Literature or Topic tree tints only its root. Now the tree's own Cards (`.node.station.native`) lie on plain stock with no coloured edge, the root alone keeps the Literature root's stock (no coloured edge either), and a Card mounted from another tree still wears its home tint and top edge. Test `gui::tests::connection_tree_own_cards_are_plain_like_any_tree`.
