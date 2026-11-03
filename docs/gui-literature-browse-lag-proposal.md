# Proposal: no long stall when browsing a Literature tree after the box changes

Date: 2026-11-03. Status: implemented as PRD #248 (slices #249–#256; decisions in `docs/gui-literature-browse-lag-prd.md`), zt 0.55.0, zt-check 0.19.2, zt-digest 0.42.1. Scope: the GUI's Card answer (`src/gui.rs` `App::card`, `render_markdown`), the Snapshot's Term meanings (`src/ops.rs` `Snapshot::term_meanings`), the term-table parser (`tools/zt-check/src/text.rs` `term_rows`), and the page's Card loading (`src/gui/app.js` `loadPointer`, `refresh`).

## 1. Summary

I browsed every Literature tree of the live box in the GUI (zt 0.54.0) and measured each move. Once the server's caches are warm, the GUI is fast: a move blocks the page for 0–126 ms, and the page holds 800–2,000 elements. One long lag remains, and it is large. **Three Card reads hung for 23–29 s and were answered at the same millisecond.** During that time the GUI showed the previous Card and gave no sign that it was waiting. When the answers came, the page jumped back to Cards the reader had already left.

The cause is on the server. The first Card read of a new Snapshot whose body holds a Term link builds `Snapshot::term_meanings`, the short meaning of every Term head in the box. That build parses a whole Term-list table once for every head in that list. On top of that, the parser itself is quadratic in the number of rows. For atkinson's Term list (748 heads, 748 rows) this does about 209 million case-folded string comparisons. A benchmark of the same code on the same body takes **40.2 s**; parsing the table once and looking each head up takes 46 ms. Each write anywhere in the box makes a new Snapshot, so the stall comes back after every write. While a digest or a model-fold session writes every few seconds, nearly every Card the reader opens waits for this build. The four request threads then fill, and even the 2 s heartbeat waits.

Two faults on the page make the stall worse. The page applies a Card answer even when the reader has moved on, so a late answer drags the reader back. The page also shows nothing while it waits.

The proposal has three parts. Part A removes the stall at its cause (§5.1–§5.3). Part B makes the page robust to any slow answer (§5.4–§5.5). Part C trims the payloads that each change still costs (§5.6–§5.7). §6 gives the tests and benchmarks that hold it in place.

## 2. How I browsed (2026-11-03, live box, read only)

- The built-in browser pane opened the live GUI at `http://127.0.0.1:4717/` (service 0.54.0, 12,442 Cards, revision 33,615). No write was made: no create, edit, move, delete, lock or Spread change. Probes on the live box carry risk (see `docs/gui-large-tree-performance-proposal.md` §3). I took precautions: one page, no forced layouts, no repeated whole-box fetches, free memory checked before the one benchmark build, and no watcher loop.
- A small script in the page recorded long tasks (`PerformanceObserver`, type `longtask`), every `fetch` with its time and size, and the element count. Each move set `location.hash` as a click does, then waited 2.5–4 s. `PerformanceResourceTiming` split each slow request into queueing time and server time.
- The moves:
  - The root, the first, middle and last top-level Cards, and one deep Card of each of the nine agent Literature trees.
  - In atkinson: its Term list `1|a`, its Cited list `1|d`, a model (`19|a` and two of its parts), Whole tree (`x`), the Table and the Constellation (`v`), the Term-uses switch, and a search (`税收`).
- The pane was hidden for most of the browse. Long tasks and request times are valid in that state; frame and paint times are not, so this report does not use them.

## 3. What I found

### 3.1 Warm moves are fast

| Move | Page blocked | Elements | Map cards drawn | Server |
|---|---|---|---|---|
| Open a tree root (hansen, hart, ebrill, atkinson) | 72–144 ms (one long task) | 870–1,915 | 26–27 | Card 8–80 ms; tree 35–181 ms |
| A move inside a tree (all nine trees) | 0–58 ms | 800–1,220 | 27–42 | Card 7–102 ms |
| atkinson model `19\|a`, then its parts | 115 ms, then 0 ms | 1,000–1,220 | 35–42 | 93–202 ms (model structure) |
| atkinson Term list `1\|a`, Cited list `1\|d` | 0–54 ms | 890–950 | 27–34 | 45–79 ms |
| Whole tree `x`, Term-uses switch | 0 ms | about 900 | 30 | — |
| Table, Constellation | 66 ms, 125 ms | about 970 | — | — |
| Search `税收` (entry mode) | 0 ms | 1,945 | — | 140 ms |

The fixes of #238 hold: no move builds more than about 2,000 elements, and no warm move blocks the page for more than 150 ms.

### 3.2 The stall: Card reads that wait 23–29 s

The page started at time 0 and fetched its overview and graph (1.24 s, the first summary build of this Snapshot). It then opened `hansen2021demand` (11 ms). The next Card reads hung. Times are page times in milliseconds:

| Request | Sent | Answered | Waited | Of which on the server |
|---|---|---|---|---|
| `card?at=hansen2021demand/1` | 23,832 | 53,010 | 29,178 | 29,173 |
| `card?at=hansen2021demand/1` (again: the page's fallback after a move to an address that does not exist) | 26,853 | 53,010 | 26,157 | 6,145 (queued 20,011 in the browser) |
| `card?at=hansen2021demand/2` | 29,846 | 53,009 | 23,164 | 23,159 |
| `card?at=hart1995firms/1` | 51,213 | 53,010 | 1,796 | 1,793 |
| `ping`, `overview` (the 4th thread was now busy) | 52,146 | 53,011 | 865 | queued |

Four facts point to one shared cause on the server:

1. All four Card reads were answered within 1 ms of each other (53,009–53,010). A hang that ends at one instant is a wait on one shared thing, not four slow reads.
2. Other requests were answered at once during the hang. These were 21 overview and ping requests (2–3 ms each), `card?at=hart1995firms` (a root, 9 ms), and a bad-address read (5 ms). So the Snapshots mutex was free, and so was at least one request thread.
3. The hung reads were all Regular Cards with Term links in their bodies. The reads that passed were roots (their index bodies hold no Term link) and an error.
4. Afterwards the same reads answered in 3–5 ms (`curl`, same Snapshot). The box had no write between 00:13:54 (last WAL write) and the end of the browse, so no SQLite lock was involved.

### 3.3 The page during the stall

- **It showed the old Card with no sign of waiting.** Each move changed the address in the location bar. The reader and the Map stayed on the previous Card for up to 29 s.
- **Late answers pulled the reader back.** When the three hansen answers arrived at 53,010, the reader was already on `hart1995firms/1`. Each answer was applied anyway. Each one switched the open tree back to hansen and fetched `tree?id=hansen2021demand` again: 3 × 261 KB at 53,013. Only then did the hart answer land.
- **The heartbeat waited too.** Once four Card reads were hung, all four request threads were taken (#244 gave the server four). `ping` and `overview` waited 865 ms in the queue. A longer stall with four waiting Card reads delays the heartbeat by the whole stall.

### 3.4 What each change still costs (not a stall, but it adds up)

| Read | Size | Server | When |
|---|---|---|---|
| `GET /api/locks` | 3.0 MB (24 Locks; the three largest carry 1,153, 460 and 1,420 member rows) | 84–110 ms, runs `tree_lock::check` | Boot and every refresh, that is after every write anywhere |
| `GET /api/card?at=atkinson2015lectures` (the root) | 1.8 MB: `inbound` and `reverse_links` 693 KB each (5,372 sources, the same list twice), `text` 366 KB | 50–80 ms | Every visit to the root |
| `GET /api/graph` | 11.3 MB | 1.26 s on a cold Snapshot | Boot only (#243 removed it from refresh) |
| `GET /api/tree?id=atkinson2015lectures` | 6.1 MB | 181 ms | Opening atkinson, or a write in atkinson |

## 4. Root causes

### R1. Term meanings parse a whole table once per head (server, the stall)

`render_markdown` (`src/gui.rs`, the Term-link branch near line 2553) asks `snapshot.term_meaning(target)` for every Term link it renders. The first ask on a Snapshot builds `Snapshot::term_meanings` (`src/ops.rs`, `fn term_meanings`), a `OnceLock` over the whole box:

```rust
for head in heads {                                   // 2,415 Term Cards; 748 under atkinson2015lectures/1|a
    ...
    let meaning = zt_check::text::term_table(&parsed.body)   // the whole list table, again for every head
        .into_iter()
        .find(|row| row.term == name)
```

So the cost is (heads of a list) × (cost of one parse of that list's table), summed over the lists. Every other Card read that renders a Term link blocks on the same `OnceLock` until the build ends. That is why the hung reads all ended at one instant (§3.2, fact 1).

### R2. The term-table parse is quadratic (zt-check)

`term_rows` (`tools/zt-check/src/text.rs`) keeps the compatibility rule "a later row of the same term, case ignored, replaces the earlier one" with a linear search over the rows read so far. Each comparison case-folds both names with `to_lowercase`, which allocates:

```rust
match rows.iter_mut().find(|known| key(&known.term) == key(&row.term)) {
```

For 748 rows, one parse makes about 280,000 comparisons and 560,000 string allocations. R1 multiplies this by 748 heads.

**Measured** (`term-meanings-bench`, release build of zt-check 0.19.1, atkinson's Term-list body of 32 KB):

| Work | Time |
|---|---|
| One `term_table` parse of the 748-row table | 38–39 ms |
| `term_meanings` as written: one parse per head, 748 heads | **40.2 s** |
| Parse once, then look each head up in a map | 46 ms |

The server's stall (29.2 s) and the benchmark (40.2 s) have the same order of magnitude. They are not equal, because the machine load and the build differ. The other lists add little: about 55–60 heads each over tables of about 60 rows.

Other callers carry the same quadratic parse, though no stall was measured in them:

- The write gate (`src/gate.rs`) parses a list once for each Term write (about 40 ms on atkinson).
- `src/term.rs` parses a list once per call.
- zt-check's own `read_terms`, `term_table` in `core.rs`, which checks each Card of a book, costs about 40 ms per check of an atkinson Card.

### R3. The cache dies with every write (server)

A Snapshot lives until the next commit by any client (`Snapshots::current` compares `PRAGMA data_version`). The Term meanings live in the Snapshot, so every write throws them away, even a write in a book that has no Term list. A model-fold session or a digest writes every few seconds, and the next Term-linked Card read pays R1 + R2 again. This is how a session working on one book makes the GUI stall on every book.

### R4. The page applies stale answers (client)

`loadPointer` (`src/gui/app.js`) awaits `api('card?at=…')` and assigns the answer to `state.card` with no check that `state.pointer` is still the address it asked for. It then fetches that Card's tree if the tree differs, and renders it. A slow answer therefore overwrites a newer move and switches the tree back. Nothing cancels the superseded request, so it keeps a request thread and a browser connection busy until it is answered. One hansen read spent 20 s queued in the browser before it was sent.

### R5. Waiting is invisible (client)

Between a move and its answer, the reader and the Map keep showing the previous Card at full ink, and nothing marks the request as pending. Under the first principles (limited attention: a state change the reader must notice gets motion; mental mapping: an object the hand reached for responds at once), a 29 s wait reads as a frozen program rather than a slow fetch.

### R6. Each change still re-reads 3 MB of Locks (client and server)

`refresh()` reads `GET /api/locks` after every change of the overview fingerprint. Most changes are Card writes, not Lock changes. The answer repeats every Lock's member rows and history, 3.0 MB for 24 Locks, and the server runs a full `tree_lock::check` for it. This is not a stall. During a write burst, though, the page re-reads 3 MB every 4 s.

### R7. A large root sends its inbound list twice (server)

A tree root that every Card links to (atkinson: 5,372 sources) sends that list twice, as `inbound` and as `reverse_links` (693 KB each). It also sends the stored `text` (366 KB), which only the writing desk uses. The reader shows 50 chips from it (#242).

## 5. Proposal

### Part A: remove the stall at its cause

**5.1 Parse each Term list once per Snapshot (R1).** Build the meanings list by list: group the heads by their list (`parent_location`), parse each list's table once into a map from term name to meaning, and look each head up in it. The build becomes linear in the size of all Term lists. Better still, make it lazy per list: `term_meaning(head)` parses only that head's list, on the first ask, and keeps the map in a per-Snapshot `Mutex<HashMap<list, Arc<…>>>` or `OnceLock` per list. A Card that links Terms of one book then never pays for the other books. Expected cost: about 40 ms for atkinson's list with the parser as it is today, and about 1–2 ms with §5.2.

**5.2 Make the term-table parse linear (R2).** Keep the replacement rule exactly, but find the earlier row through a `HashMap<String, usize>` keyed by the case-folded name: fold each row's name once. The rows, their order and the replacement result stay byte-identical, so R6 and every reader agree as before. Apply the same change to `read_terms`. This fixes the GUI, the write gate, `term.rs` and zt-check's per-Card checks together. It is a zt-check release (0.19.2) with the existing term-table tests plus a large-table test (§6).

**5.3 Keep parsed lists across Snapshots (R3).** Key the per-list map by the list Card's body fingerprint, in a small cache on `Snapshots` (outside the Snapshot), so a new Snapshot reuses every list whose body did not change. A write in hart then costs no Term-list parse at all, and a write in atkinson's Term list costs one parse of that list. With §5.1 and §5.2 in place the parse is cheap, so this step is optional. It is worth doing only if a benchmark after §5.2 still shows the build above 50 ms on a cold Snapshot.

No other part of the Card answer should run a whole-box computation on first use. Section 6 adds a guard that fails when a cold Card read on the benchmark box takes more than 500 ms.

### Part B: a page that copes with any slow answer

**5.4 Apply only the latest answer (R4).** Give each `loadPointer` call a sequence number (or an `AbortController`). When an answer arrives for an older sequence, drop it: no `state.card` change, no tree fetch, no render. Abort the superseded request so it frees its connection. Apply the same guard to the tree fetch inside `loadPointer` and to the study desk's `cardAt` reads. The reader then always ends on the last Card asked for, however the answers are ordered.

**5.5 Show that a Card is on its way (R5).** If an answer has not arrived 300 ms after a move, mark the wait where the reader's attention already is, without adding a new element:

- The Map's target card takes the `focus` lift at once, from the layout the page already has (it knows the outline and the target's place), so the hand's move is answered at once.
- The reader keeps the previous Card, faded (lower ink, as a sheet being put away). A thin pencil line runs along the top edge of the reader, the one moving thing on screen.
- At 5 s, a quiet line in the reader says the box is busy and names the address being fetched, with no dialog and no toast.

The old Card never looks current while a new one is pending. The motion stays within the 200–500 ms range for the lift and the fade.

### Part C: lighter answers on every change

**5.6 Read the Locks only when they change (R6).** Add a `locks` fingerprint to `/api/overview`: a hash of the Lock rows, events and recorded faults, made with the overview once per Snapshot. `refresh()` reads `/api/locks` only when that fingerprint moved. The integrity check point that opening the Lock panel records (R292) stays with the panel's own read (`loadLocks` already calls `api('locks')`). Expected: a Card write no longer moves 3 MB.

**5.7 Send a large root's inbound list once, and only its head (R7).** Make `inbound` the union the page navigates by, and send it the way #242 sends sides: `inbound_count`, the first 50 sources, and the full list on `?inbound=all`. Derive `reverse_links` and `term_uses` on the page from `inbound` with a role field, or send them as counts plus their first 50. Send `text` only on `?text=1`, which the writing desk asks for when it opens. Expected: the atkinson root goes from 1.8 MB to about 20 KB. The MCP `get_card` answer does not change.

## 6. Tests and benchmarks

The tests are test-first, at the seams these steps change:

- **zt-check `text`:** a 2,000-row term table, with case-variant repeats placed late, parses to the same rows as today and in under 20 ms in a release build. Old fixtures stay byte-identical (`term_table`, `term_rows_raw`, `read_terms`).
- **`Snapshot::term_meaning`:** with a Term list of 1,000 heads, every head's meaning equals the meaning from its own list row. The first `term_meaning` on a fresh Snapshot takes under 100 ms (debug build bound, generous). A list with no table and a head with no row both give no meaning.
- **GUI route:** on a fixture box with a 1,000-head Term list and a Regular Card that links one Term, a cold `/api/card` (the first read after a write) answers in under 500 ms. Two Card reads sent together on a cold Snapshot both answer, and a `ping` sent with them answers in under 100 ms.
- **Page (`layout.js`/`app.js` source and behaviour tests):** two `loadPointer` calls whose answers arrive in reverse order end on the second Card, with one tree fetch. The superseded request is aborted. The pending mark appears after 300 ms and clears on the answer.
- **Locks:** a Card write leaves the overview's `locks` fingerprint unchanged, and the page does not read `/api/locks`. A Lock, unlock, note or recorded fault moves it.
- **`scripts/gui-bench`:** add `cold-card.mjs` for a test box. It makes one write through the test box's MCP server, then times the first `/api/card` of a Term-linked Card and the next `ping`. It reports both and exits 1 above the bounds. It refuses port 4717, like `probe.js`.

The before and after numbers come from a test box, never the live box. The test box carries atkinson's Term list and one Term-linked Card. Section 3.2 gives the before; the after should show a cold Card read under 500 ms.

## 7. Order and size

| Step | Size | Removes |
|---|---|---|
| 5.2 linear term-table parse (zt-check 0.19.2) | small | most of R2; also speeds the write gate and digest checks |
| 5.1 one parse per list, lazy per list | small | R1: the stall drops from about 40 s to under 50 ms |
| 5.4 latest answer only, abort the rest | small | R4 |
| 5.5 pending mark | small to medium | R5 |
| 5.6 Locks fingerprint | medium | R6 |
| 5.7 lean root answer | medium (API shape; MCP unchanged) | R7 |
| 5.3 cache lists across Snapshots | only if needed | the rest of R3 |

Steps 5.2 and 5.1 alone remove the lag the owner sees. Steps 5.4 and 5.5 make the page correct and honest under any future slow answer. Steps 5.6 and 5.7 trim what remains.

## 8. Limits of this report

- I observed the stall once on the live box, at the first Term-linked Card read of the Snapshot that was current when the page opened. To see it again I would have had to write to the live box, which I did not do. The offline benchmark reproduces the cost from the same code and the same list body. The proposed `cold-card.mjs` measures the stall end to end on a test box.
- During the browse, another session was folding atkinson's models, which explains the backup `backups/zt-before-atkinson2015lectures-fold.sqlite3` and the restructured Term list. Its last write was at 00:13:54, before this browse began, and the browse did not interact with it.
- The benchmark's scratch project lives outside the repository. It built into `D:\zt-check-target`.
