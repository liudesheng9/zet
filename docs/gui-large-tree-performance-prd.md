# PRD: A GUI that stays responsive with very large successor lists

Design record: `docs/gui-large-tree-performance-proposal.md` (measurements, incident, root causes R1–R5).

## Problem

The GUI locks up when the open Card, or a Card on its path, has a very long list of successors. On the live box (12,442 Cards; `atkinson2015lectures` 7,282 Cards), the Term list `atkinson2015lectures/1|a` (1,565 outline children) and the Cited list `…/1|d` (1,589) make the Map build 1,800–3,397 cards and up to 76,000 DOM elements. Each Pointer move then blocks the page for 1.0–2.0 s.

- **R1. Unbounded focus lens.** `focusLens` shows every outline child of every Card on the path (`src/gui/layout.js:113-118`).
- **R2. Every move touches every shown card several times.** These are the `className` writes before the change check, a forced layout from `clientWidth`, and a glide that writes `width`/`height` on every card on every frame, which is about 280 ms per pass at 1,800 cards. Per-card timers add to that.
- **R3. Heavy cards.** Every slip typesets its TeX title with KaTeX, about 37 elements per card in the Term list.
- **R4. Whole generated lists in the reader.** The Cited list body is 9,832 elements and takes 231 ms. `/api/card` sends 1,589 `sides`.
- **R5. Whole-archive reloads.** Any change makes the page refetch `/api/graph` (10–11.6 MB, 1.5–1.8 s) and the open tree (6–7 MB). The GUI server answers one request at a time, so Card reads wait behind those builds.

During the measurement on 2026-11-02 the machine locked up. My probe of this page overlapped with 54 stale digest watcher loops, each running `zt-digest ledger status`, which costs 7.5 s CPU and 337 MB and prints 25 MB for four counts. The ledger also had a 2.03 GB WAL. The watchers had no upper bound in number or lifetime.

## Goal

1. A Pointer move costs work in proportion to what the user can see, not to the length of any sibling list or the size of the tree. The target is a main-thread block under 100 ms per move on an atkinson-sized tree.
2. The Map keeps the whole structure readable. Long lists recede into physical bundles of slips that show how much they hold and open on demand (GUI first principles: limited attention, mental mapping).
3. A change elsewhere in the box costs the page a few KB, not the whole archive. A Card read never waits behind a whole-archive build.
4. A run watcher cannot multiply or outlive its run, and a status read costs milliseconds.

## Decisions

- **D1 Sibling window.** A Card whose outline children number more than **30** shows a window of them: the anchor child, **8** siblings before it, **8** after it, and the list's first and last child. The anchor is the child on the path to the Pointer (or the Pointer itself). For the focus's own children, or a list off the path, the anchor is the first child. The hidden runs between them fold into **bundles**. Lists of 30 or fewer are unchanged.
- **D2 Bundles are physical.** A bundle is a slip-sized object in the column where its slips would lie, drawn as a thick stack of slip edges. It shows the count ("+612") and the address range it holds. A click riffles it: the window moves to the slips just behind the bundle, and the Pointer stays where it is. Moving the Pointer re-anchors the window. Keys (↑/↓) walk through a list one slip at a time, and the window follows. The bundle keeps the Folgezettel folds (`+N` on a Card) as they are: those count descendants, while a bundle counts siblings.
- **D3 Whole tree (`x`)** unfolds every branch but keeps the sibling window, so a 1,589-entry list is one bundle in the overview, not a 50,000-pixel column.
- **D4 Draw only what the camera sees.** The lens still lays out every shown Card, but the Map creates DOM only for Cards and edges within the viewport plus one screen of margin. Panning and zooming add and remove them.
- **D5 Cheap moves.** `className` is written only when it changes. The viewport size is read once per resize. The glide sets each card's final size once and animates only `transform`. Only on-screen cards glide, and when more than 150 move at once, the move snaps (as it already does with reduced motion). One timer per render clears the `entering`/`leaving` marks. `.node` gets `contain: layout style` (not `paint`, because the pointer label, part labels and Table tab draw outside the card).
- **D6 Titles typeset once.** Slips keep their math (readability wins over a few elements). Each distinct title is typeset once and the nodes are cloned from a cache.
- **D7 Long lists in the reader.** A Markdown table of more than 60 rows shows its first 50 rows and a "Show all N rows" control. The rows are cut off-DOM before insertion. Chip rows of more than 60 entries (Referred by, Links out) show 50 and a "+N more" control. Term uses build their chips only when the disclosure opens.
- **D8 `/api/card` sides.** `/api/card` answers `sides_count` and only the first Side successor. `?sides=all` returns the full list, for the reorder step. The MCP `get_card` reply (`src/read.rs`) is unchanged.
- **D9 Summary cache per Snapshot.** Each Card's serialized summary is built once per Snapshot. `/api/tree` and `/api/graph` are assembled from the cached strings. Each tree gets a fingerprint, the hash of its serialized summaries, so it is exact by construction, including inbound counts and Lock facts. `/api/overview` lists it per tree.
- **D10 Incremental refresh.** When the box fingerprint changes, the page refetches only the trees whose fingerprint changed (and the trees that were added) and splices them into its node index. The open tree is reloaded only if its fingerprint changed. The page falls back to `/api/graph` when the overview carries no tree fingerprints.
- **D11 Read pool.** The GUI server runs four request threads. Writes (`POST` routes) are serialized by one in-process lock, and Card writes still take the archive edit lock as before. The Snapshot build stays under its mutex, so concurrent readers wait for one build instead of making several.
- **D12 Counts-only ledger summary.** `ledger status -Summary` reads the counts with SQL and parses no unit JSON. Its fields and values equal the summary of the full status, and a test holds the two paths equal.
- **D13 Bounded WAL.** Writable ledger connections set `journal_size_limit` (64 MiB), so a checkpointed WAL shrinks. `ledger checkpoint -Out` truncates the WAL on demand.
- **D14 `ledger watch`.** `zt-digest ledger watch -Out <dir> [-Journal <path>] [-Every <s>] [-MaxHours <h>]` is the one supported run watcher. It holds a per-run lock file with its PID (a second watcher for the same run refuses, and a dead holder's lock is taken over). It prints a line on a layer or fault change. It exits when the ledger reports `complete`, when the journal reports `halted`, or after `-MaxHours` (default 12). `docs/agents/workflow-principles.md` requires it.
- **D15 Safe probes.** GUI benchmarks run on a test box made with `copy-tree.mjs`, never on the live box. `scripts/gui-bench/` holds a Node lens benchmark (any tree, any focus: lens size under its bound) and a bounded browser probe with a fixed move list.
- **D16 Versions.** zt 0.53.1 → 0.54.0. zt-digest 0.41.25 → 0.42.0.

## Seams under test (TDD)

- `ZtLayout.focusLens(outline, focus, { windows, expandAll, links })`: nodes, bundles, folds and bounds (pure, run under Node from `src/gui.rs` tests).
- `ZtLayout.visible(nodes, rect, margin)` and `ZtLayout.glidePlan(from, to, visibleRect, limit)`: culling and the snap rule (pure).
- `ZtLayout.foldRows(count)`: the reader's fold rule (pure).
- HTTP API through `App::route`: `/api/card` (`sides_count`, `sides=all`), `/api/overview` tree fingerprints, and `/api/tree`/`/api/graph` equal to the uncached summaries.
- The server: two concurrent readers are both answered while a slow read is in progress.
- `zt-digest ledger status -Summary` equals `status_summary(full)` on fixture ledgers. `ledger watch` refuses a second instance and exits on completion, on a halted journal and on max hours. `journal_size_limit` is set on writable connections.
- The running GUI on a test box: lens sizes, long tasks per move and DOM counts, through `scripts/gui-bench`.

## Slices

- [ ] #239 S1 Lens sibling window and bundles
- [ ] #240 S2 Moves cost only what changes
- [ ] #241 S3 The Map draws only what the camera sees
- [ ] #242 S4 Long lists in the reader; `/api/card` sides
- [ ] #243 S5 Summary cache, tree fingerprints, incremental refresh
- [ ] #244 S6 GUI read pool
- [ ] #245 S7 Run watcher and ledger: counts-only summary, bounded WAL, `ledger watch`
- [ ] #246 S8 Bounded GUI benchmark and test-box verification

## Out of scope

- A different visual form for Term lists and Cited lists on the Map, beyond the bundle.
- Server-side gzip.
- The daemon heartbeat that stopped at 19:53:51 on 2026-11-02: #247 found it is no fault, because the daemon logs a heartbeat only while a digest run's marker is fresh.
