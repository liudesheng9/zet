# Proposal: a GUI that stays responsive with very large successor lists

Date: 2026-11-02. Status: implemented as PRD #238 (slices #239–#246; decisions in `docs/gui-large-tree-performance-prd.md`); the heartbeat question was answered in #247 (not a fault). Scope: the GUI Map, the reader, the GUI data path (`src/gui.rs`, `src/ops.rs`, `src/gui/app.js`, `src/gui/layout.js`, `src/gui/app.css`), and the digest watcher that overlapped with the incident (`tools/zt-digest/src/ledger.rs`, session watcher scripts).

## 1. Summary

The owner reported that the GUI locks up when the box holds more than about 1,000 Cards and a Card with a very long list of Direct or Side successors is opened. The live box now holds 12,442 Cards. Its largest tree, `atkinson2015lectures`, holds 7,282 Cards. Two Cards in it have very long successor lists: the Term list `atkinson2015lectures/1|a` (748 Term Cards, 1,565 outline children) and the Cited list `atkinson2015lectures/1|d` (1,589 cited works).

The fault is in the browser, not the server. The server answers one Card in 7–30 ms. The Map's focus lens has no upper bound on how many siblings it shows. When any Card on the path from the root to the open Card has 1,500 successors, all of them are built as DOM cards. Each move of the Pointer then touches every one of those cards in several full style-and-layout passes, at about 0.15 ms per card per pass. Opening a Card in the Term list builds 1,806 map cards and 66,000 DOM elements and blocks the page for 1.0–1.7 s. Opening the Literature root's first Card builds 3,397 map cards and 76,000 elements. A second, slower data path adds to this. Every change in the box makes the page fetch the whole archive again (10–11.6 MB) and the open tree (6–7 MB).

While I measured this on 2026-11-02, the whole machine locked up. Section 3 sets out that incident. My probe put a 76,000-element page into the Claude desktop app's browser pane and ran about 25 forced full-layout passes. At the same time, 54 forgotten watcher loops from the finished atkinson digest session were running a heavy ledger read in parallel, up to 19 at once. The probe and the watchers overlapped. The watchers have been stopped.

The proposal has three parts. Part A bounds what the Map draws, so the cost of a move no longer grows with the length of a sibling list (§5.1–§5.5). Part B makes the data path incremental (§5.6–§5.8). Part C makes watchers and probes safe for the machine (§5.9–§5.11).

## 2. Measurements (2026-11-02, live box, read-only)

All reads went through the GUI's own HTTP API. Nothing was written to the box. The browser numbers come from the built-in browser pane while the pane was hidden. Long tasks and forced synchronous style/layout times are valid in that state. Frame and paint times are not valid, because a hidden pane throttles its frames, so this report does not use them as evidence.

### 2.1 The largest successor lists

| Card | What it is | Outline children | Map cards when a child is open |
|---|---|---|---|
| `atkinson2015lectures/1\|d` | Cited list, 1,589 works | 1,589 | 1,830 |
| `atkinson2015lectures/1\|a` | Term list, 748 Terms | 1,565 | 1,806 |
| `atkinson2015lectures` (root) | Literature root, Direct run | 229 | — |
| `hart1995firms/11` | Cited list | 224 | 236 |
| `ebrill2001modern/24` | Cited list | 223 | 248 |

The worst focus-lens size in `atkinson2015lectures` is 3,397 Map cards (Pointer at `atkinson2015lectures/1`, whose four children are few enough that the lens also shows all 3,154 grandchildren). A list of about 230 successors (`hart1995firms/11`) is still comfortable. One move there costs a single 151 ms long task.

### 2.2 Server

| Request | Time | Size |
|---|---|---|
| `GET /api/card` (Term list, Cited list, ordinary Card) | 7–30 ms | 23 KB – 1.26 MB |
| `GET /api/tree?id=atkinson2015lectures` | 0.84 s alone; up to 3.96 s queued behind other requests | 6.3–7.1 MB |
| `GET /api/graph` | 1.47–1.76 s | 10.2–11.6 MB |

The server is not the cause of the stall on a move. Two server facts still matter. First, the `zt-gui` thread answers one request at a time (`src/gui.rs:179`), so a 1.5 s graph build delays every Card read behind it. Second, `/api/card` for the Cited list is 1.26 MB, because `sides` lists all 1,589 Side successors with their Lock facts (`src/gui.rs:1313`, `src/ops.rs:1149`). The reader uses only `sides[0]` (the → key) and the reorder step.

### 2.3 Browser: what one move costs

| Move | Map cards | DOM elements | Long tasks (main thread blocked) |
|---|---|---|---|
| into `hart1995firms/11` (224 successors) | 236 | 2,758 | 151 ms |
| inside it, to `hart1995firms/11\|5` | 236 | 2,759 | none |
| into `atkinson2015lectures/1` (fresh tree) | 3,397 | 73,230 | 920 ms, after a 1–4 s tree fetch |
| `1\|a\|5` (inside the Term list) | 1,806 | 72,243 | 1,030 + 562 + 154 ms |
| `1\|a\|6` | 1,806 | 72,244 | 667 + 181 + 100 + 127 ms |
| `1\|d` (open the Cited list) | 1,830 | 19,321 | 1,207 + 839 ms |
| `atkinson2015lectures/2` (leave the lists) | 251 | 1,793 | 331 + 92 ms |

The layout functions are not the cost. `ZtLayout.outline` on 7,282 Cards takes 23 ms. `focusLens` takes 3–7 ms, and 13 ms even for the whole tree unfolded. The time goes to the DOM.

Synthetic passes over the 1,830 cards of the Cited-list view measure the price of one full pass (forced synchronously, mean of three):

| One pass over every shown card | Time |
|---|---|
| toggle one class (what `content()` and the `in`/`out` marks do) | 280 ms |
| set width on every card (what each glide frame in `paint()` does) | 281 ms |
| the same width pass with `contain: layout style paint` on `.node` | 168 ms |
| the same width pass with the KaTeX titles replaced by text | 175 ms |
| `content-visibility: auto` on `.node` | 234–301 ms, no gain (the cards are absolutely placed, so the browser still visits each one) |
| insert the Cited list's rendered body (325 KB HTML, 9,832 elements) into the reader | 231 ms |

The Term-list view is heavier per card. Many Term titles carry TeX, so the 1,806 cards hold 1,061 KaTeX trees, about 37 elements per card and 66,415 elements in `#nodes`.

## 3. The incident during the probe: why the whole machine locked

The owner reported a system-wide lock while I probed the GUI page. The record below covers what is measured, what is inferred, and which part was my own doing.

### 3.1 What ran at the same time

1. **My probe, about 20:08–20:14.** I opened `atkinson2015lectures/1` in the built-in browser pane of the Claude desktop app and moved through the Term list and the Cited list about 12 times. Each move blocked the pane's page for 0.5–2 s. I then ran synthetic benchmarks that forced about 25 full style/layout passes over 1,800 cards (0.17–0.3 s each). The page held up to 76,641 DOM elements. Inside the page I also fetched `/api/graph` (10 MB), the atkinson tree (6.3 MB) and the Cited-list Card (1.1 MB), and built a 9,832-element test node. Outside the browser I fetched the graph and the tree twice more with `curl`. I also ran one Node script that computed the focus lens for every Card of every large tree, which is about 12,000 lens computations on one core for under a minute. The pane is part of the desktop app, so while its page was blocked the app's own window was slow too, and the user saw that as a lock.
2. **54 watcher loops left from the finished atkinson digest session (`cdb8e612`), started between 2026-11-01 14:29 and 2026-11-02 19:25.** Each loop (`scratchpad/watch3.sh`) runs `node st.mjs`, which runs `zt-digest ledger status -Out D:/zt-digest/atkinson2015lectures`, then sleeps 240 s, forever. The run they watched is finished, so none of them had any use. At 20:12:45–20:13:34, 19 of these status reads were in flight together, each about 177 MB of working set and about 0.6 of a core. Free RAM had fallen to 8.8 GB and Windows' Memory Compression held 1.6 GB.
3. **The zt daemon's heartbeat lines stopped at 19:53:51.** This is not a fault (#247). The daemon logs a heartbeat only while a digest run's marker is fresh, and the atkinson run's marker was last renewed at 19:49:25 with a 300 s lifetime. The `zt-gui` thread kept answering requests throughout.

At 20:14 the 54 loops were killed with their process trees. Two minutes later free RAM was 12.0 GB, CPU load was 18%, and no `zt-digest` or `bash` process remained. Windows logged no resource-exhaustion or kernel-power event between 19:30 and 20:20.

### 3.2 Why the watchers piled up

One `ledger status` read was timed alone afterwards, at low priority on an idle machine. It took 7.9 s wall, 7.5 s CPU and 337 MB peak RAM, and it printed 25 MB of JSON. `st.mjs` uses only the run's counts from that output. The read is heavy for three reasons.

- `status` (`tools/zt-digest/src/ledger.rs:5879`) loads every unit row and parses each unit's stored `inputs`, `fault`, `progress` and `result` JSON. `-Summary` (`ledger.rs:5860`) shortens the output but builds the same full status first, so it costs the same CPU and memory.
- The atkinson ledger is a 58 MB database with a **2.03 GB write-ahead log** that was last written at 19:49. Every new reader opens that WAL.
- A reader that keeps a snapshot open stops a checkpoint from resetting the WAL. Dozens of loops that keep reading the same ledger therefore keep the WAL from shrinking, and a larger WAL makes each read slower. This feedback is likely, but this report did not measure it.

At steady state, 54 loops × 7.5 s CPU every ~248 s needs about 1.6 cores. A 32-core machine can carry that. The pile-up of 19 concurrent reads means each read was taking about 85 s instead of 8 s. At 0.6 of a core each, CPU starvation does not explain that alone. The likely brake is I/O and memory: 19 processes reading the same 2 GB WAL and holding 177–337 MB each, while my probe was pushing the desktop app and the zt service hard at the same moment. I cannot separate the two loads exactly from the evidence that remains. The probe was a real share of the lock. The watchers turned it from a slow pane into a slow machine.

### 3.3 Lessons applied in this proposal

- A GUI probe on the live box must be bounded. It must not open the largest tree fully in the desktop app's pane, and it must not repeat whole-archive fetches (§5.11).
- A watcher must end when its run ends, must have at most one instance per run, and must read a cheap counts-only status (§5.9, §5.10).
- The heartbeat that stopped was the end of the digest run, not a fault (#247).

## 4. Root causes of the GUI stall, ranked

### R1. The focus lens has no bound on siblings (main cause)

`focusLens` (`src/gui/layout.js:105`) shows every outline child of every Card on the path from the root to the Pointer (`layout.js:113-116`). It also shows the focus's grandchildren whenever the focus has 14 children or fewer (`layout.js:118`). The lens was designed in October 2026, when the largest tree held 99 Cards. The generated Term lists and Cited lists now give single Cards 200–1,600 successors. Every Card inside such a list carries the whole list into its view. `atkinson2015lectures/1`, whose four children are the lists, carries both lists (3,397 cards). A user cannot read 1,800 slips on one screen. This breaks the attention-first rule as well as the frame budget.

### R2. Every move touches every shown card several times

The Map's `render` (`src/gui/app.js:1618`) and its glide do work proportional to the number of shown cards, several times per move.

- `content()` writes `el.className` on every card before it checks whether the card changed (`app.js:1462` and `:1471`). Each write invalidates the style of the card and its subtree. At 1,800 cards this is one 280 ms pass.
- `cameraTarget` reads `#viewport.clientWidth` right after the DOM changes (`app.js:1504`). That read forces a synchronous style and layout of the whole plane.
- `glide` (`app.js:1597`) calls `paint()` on each animation frame for 360 ms. `paint()` writes `transform`, `width` and `height` on every card (`app.js:1527-1533`). The width and height writes force layout. At 1,800 cards one frame costs 200–500 ms, so the "glide" shows two or three jerks and blocks input between them.
- Every new card gets its own 400 ms timer and every leaving card its own 240 ms timer (`app.js:1683`, `:1694`). A fresh tree starts 3,397 timers and 3,397 `appear` animations.
- Each `.node` carries a transition on six properties, layered box-shadows and gradient paper rules (`app.css:466-548`). These cost paint on every card that moves. Paint was not measurable in the hidden pane, so this cost is still unmeasured.

### R3. Each card's DOM is heavy where titles carry TeX

Slip-level cards render their titles through KaTeX like full cards do. In the Term-list view that is 1,061 KaTeX trees and about 37 elements per card. Without KaTeX the same width pass costs 175 ms instead of 281 ms.

### R4. The reader renders whole generated lists

The Cited list's body is a 325 KB table. Inserting it into the reader builds 9,832 elements and costs 231 ms. The Term list is similar (57 KB). The Card's `sides` list (1,589 references, 1.26 MB of JSON) is sent with it, although the reader uses one entry.

### R5. Any change reloads the whole archive

`watchForChanges` polls `/api/overview` every 4 s (`app.js:7733`). A changed fingerprint calls `refresh()` (`app.js:578`). That refetches `/api/graph` (10–11.6 MB, 1.5–1.8 s), drops the tree, refetches it (6–7 MB for atkinson), rebuilds every index in `setNodes` and redraws everything. The server builds each Card summary afresh for every request (`ops.rs:1003`). A summary parses the Card's Links twice and does not cache anything per Snapshot. While a digest or idea-grow run writes Cards, this can happen every 4 s. The server's one request thread then queues the user's Card reads behind the graph build.

## 5. Proposed changes

Each change keeps the GUI principles the owner set: emphasize the focus and recede the rest, fast deterministic layouts, and physical mapping including motion.

### Part A: bound what the Map draws

**5.1 A sibling window in the focus lens (`layout.js`).** When a Card's outline children exceed a cap (proposed 24), the lens shows a window of them: the child on the path or the focus itself, eight siblings before it and eight after it, plus the first and the last child. The rest fold into two fold stubs, for example "+612 above" and "+940 below". The focus's own children follow the same rule. The grandchildren rule (14) stays. A stub is a real slip-shaped node of fixed height that looks like a thick bundle of slips held together, so it keeps the physical mapping. Clicking a stub moves the window to that part of the list, and a long press or right-click opens a jump list with address and title search. Expected effect: a view inside the Term list drops from 1,806 cards to about 40, and the cost of a move no longer depends on how long the list is. `layout.js` is pure and loads as CommonJS, so the bound can be tested in Node without a browser.

**5.2 Viewport culling as the safety net (`app.js`).** "Whole tree" (`x`, `expandAll`) still shows every Card of the tree on purpose: 7,282 in atkinson. The Map should create DOM only for cards whose geometry meets the camera's visible rectangle plus one screen of margin, and create or remove cards as the camera pans. The layout itself stays complete, so folds, edges and counts are unchanged. This also caps the cost of any future list that the window misses.

**5.3 Cheaper moves (`app.js`, `app.css`).**
- Write `className` only when it changed, and move the signature check before the write.
- Read the viewport size once per resize, not after each DOM change.
- Animate the glide with `transform` only. Set a card's final width and height once, at the start of the glide, and draw the size change as a `scale` in the transform. Frames then cause no layout.
- Glide only cards whose geometry changed and that are on screen. Above a threshold (proposed 150 moving cards) the move snaps, as `prefers-reduced-motion` already does.
- Replace the per-card timers with one timer per render that clears every `entering` or `leaving` mark of that render.
- Add `contain: layout style paint` to `.node` (measured: 281 → 168 ms for a width pass). Drop the six-property transition on slips, which never lift.

**5.4 Light titles on slips (`app.js`).** A slip shows its title as text, with TeX left as source or shortened. Full and near cards keep KaTeX. Rendered title nodes are cached by title string and cloned, so the same title is never typeset twice.

**5.5 Long generated bodies in the reader (`app.js`, `src/gui.rs`).** The reader shows the first 50 rows of a Term or Cited list table, then a "Show all N rows" control that inserts the rest on request. `/api/card` sends `sides_count` and the first side only; the reorder step asks for the full list through a separate route or a `sides=all` parameter.

### Part B: an incremental data path

**5.6 Cache summaries per Snapshot (`src/ops.rs`, `src/gui.rs`).** Build each Card's summary once per Snapshot, as `fingerprint()` already does with a `OnceLock`. Keep the serialized JSON of `/api/graph` and of each tree for the life of the Snapshot. Repeated reads then cost a copy, not a rebuild.

**5.7 Refresh only what changed (`app.js`, `src/gui.rs`).** `/api/overview` already lists the trees. It should also carry a fingerprint per tree. On a changed box fingerprint, the page refetches the open tree only if that tree's fingerprint changed, and it refreshes the graph only when a view that needs it is open (Constellation, Table, search, link completion), or lazily on its next use. A digest writing into another book then costs the page a 4 KB overview read.

**5.8 Read concurrency on the server (`src/gui.rs`).** Serve read routes from a small pool (two to four threads) that share the `Arc<Snapshot>`. Writes stay serialized under the edit lock as now. A Card read then never waits behind a graph build.

### Part C: watchers and probes that the machine can carry

**5.9 A counts-only ledger status (`tools/zt-digest/src/ledger.rs`).** `ledger status -Summary` should read the counts with `SELECT layer, state, COUNT(*) FROM units GROUP BY layer, state` and the few `meta` rows it reports. It should not parse any unit JSON. Expected cost: milliseconds instead of 7.5 s CPU and 337 MB. Watchers and drivers use this form.

**5.10 One watcher per run, stopped with the run.** The watcher pattern in `docs/agents/workflow-principles.md` should require four things. The watcher writes a PID file per run. It refuses to start when a live one already exists. It exits on its own when the run's journal shows the run halted or finished, or when the ledger reports `complete`. The session stops it before it reports the run done. The ledger writer should checkpoint the WAL with `TRUNCATE` at the end of a run, so a finished ledger does not keep a 2 GB WAL.

**5.11 Safe GUI probes.** GUI measurements use a test box: a copy of the archive under a scratch root with its own service and port, never the live box (this agrees with the standing rule for pilots). A probe script states its bounds before it runs: at most one whole-archive fetch, no `expandAll` in the desktop pane, a fixed list of moves. It runs in a visible pane, so frame and paint times are valid. The probe used for this report becomes `scripts/gui-bench/probe.js` with those bounds built in.

## 6. Validation

- **Node test for the lens bound.** For every focus in a synthetic tree with one Card of 1,600 Side successors and one Literature root with 800 Direct successors, `focusLens` returns at most a fixed number of nodes (proposed 120). Fold counts add up to the tree's size.
- **Browser acceptance on a test box with the same shape as atkinson (7,000 Cards, a 1,600-entry list).** A move inside the list blocks the main thread for less than 100 ms in total. No frame of the glide is longer than 50 ms. "Whole tree" pans at no more than 50 ms per frame. Opening the Cited list in the reader blocks for less than 100 ms.
- **Refresh.** With the open tree unchanged and a write elsewhere, a refresh transfers less than 50 KB and blocks for less than 50 ms.
- **Ledger.** `ledger status -Summary` on the atkinson ledger finishes in under 200 ms and under 50 MB.

## 7. Slices

1. Lens sibling window and fold stubs (`layout.js`, Node tests, stub interaction in `app.js`).
2. Cheaper moves: className guard, one viewport read, transform-only glide, snap threshold, one timer, CSS containment.
3. Light slip titles and the title cache.
4. Reader: folded long list bodies; `/api/card` sends `sides_count`.
5. Summary and JSON cache per Snapshot; per-tree fingerprints; refresh only what changed.
6. Read thread pool in the GUI server.
7. Viewport culling for "Whole tree".
8. Counts-only `ledger status -Summary`; end-of-run WAL checkpoint; watcher rules in the workflow principles.
9. Bounded GUI probe script and test-box recipe.

Slices 1 and 2 remove the reported stall. Slice 8 removes the condition that made the probe lock the machine. The other slices are independent and can follow in any order.

## 8. Open questions for the owner

1. Window size: 24 siblings shown, eight before and eight after the path child. Is that the right amount of context?
2. Fold stub interaction: should a click move the window, or open a jump list first?
3. Should "Whole tree" stay unlimited (with culling), or should it also fold lists longer than the cap?
4. Should a Term list or Cited list look different on the Map, for example as a card box or index drawer instead of a long run of slips?
5. (Answered in #247: the heartbeat logs only during a digest run, so it stopped when the atkinson run's marker expired.)
