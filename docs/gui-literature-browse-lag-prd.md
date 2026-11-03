# PRD: reads that never wait for writes, and no Term-meaning stall

Date: 2026-11-03. Source: `docs/gui-literature-browse-lag-proposal.md` (browse of the live GUI 0.54.0, root causes R1–R7) and the owner's follow-up: "the edit process holds a write lock on the database, but the read shouldn't use the same lock. It should use a read lock and should be much looser than the write lock."

## Problem

While another client writes to the box, the GUI stalls. One Card read waited 29 s, and four reads were answered at the same millisecond. The page then jumped back to Cards the reader had left. The causes, from the proposal and a review of the locks:

- **R1/R2.** The first Term-linked Card read of every Snapshot builds the Term meanings. That build parses a whole Term-list table once per head, and the parse is quadratic: 40.2 s for atkinson's 748 heads. Every reader that renders a Term link waits for that one build.
- **R3/L1.** The GUI's Snapshot cache is one exclusive `Mutex`. Every read takes it, and the reader that finds the box changed rebuilds the Snapshot while holding it (about 1 s for 12,442 Cards). Every other read waits for that rebuild, although the previous Snapshot is still a consistent state to answer from.
- **L2.** Reads and writes share the GUI's four request threads. A POST takes the write mutex and then waits up to 20 s for the edit lock while another client writes. A second POST waits for the mutex inside a request thread. Four waiting POSTs hold every thread, and no read is answered.
- **L3.** `zt dump` only reads, but it takes the exclusive edit lock. It refuses at once while any client writes, although its single read transaction already gives one consistent view (WAL).
- **R4/R5.** The page applies a Card answer after the reader has moved on, and it shows nothing while it waits.
- **R6/R7.** Each change re-reads 3 MB of Locks, and a large root answers 1.8 MB (its inbound list twice, and the stored text).

What is already right: SQLite runs in WAL mode, so readers use deferred read transactions and never wait for a writer, and a writer never waits for a reader. The edit lock (`zt.edit.guard`) is taken by writers only, apart from L3 and from the Lock panel's read when it must record a new fault (a write, which stays).

## Decisions

- **D1 (zt-check).** The term-table parse folds each row's name once and finds an earlier row of the same term through a map. The rows, their order and the replacement rule stay exactly as they are. The same applies to `read_terms`.
- **D2.** `Snapshot::term_meanings` parses each Term list once: the heads are grouped by list, and each head is looked up in its list's parsed rows.
- **D3 (read lock).** The Snapshot cache becomes single-flight with stale reads:
  - The lock is held only to compare `data_version` and to take or install a Snapshot, never while a Snapshot is read.
  - The first reader that finds the box changed builds the new Snapshot outside the lock. Readers that arrive while it builds are answered from the previous Snapshot at once.
  - A reader that finds no Snapshot at all, or a Card read whose address the previous Snapshot lacks, waits for the build.
  - The overview is cached per Snapshot as before.
- **D4 (write lane).** GUI POSTs run on one writer thread. A request thread reads the POST's body, hands the request to the writer lane, and goes back to reads. Writes stay one at a time, in arrival order, so the write mutex is no longer needed.
- **D5.** `zt dump` reads in one read transaction without the edit lock.
- **D6.** The page applies only the answer to its latest Card request, and aborts superseded requests. A Card that takes more than 300 ms is marked pending: the reader fades, and a pencil line runs along its top edge. At 5 s a quiet line names the address being fetched.
- **D7.** `/api/overview` carries a `locks` fingerprint of the Lock rows, events and recorded faults. `refresh()` reads `/api/locks` only when it moved.
- **D8.** `/api/card` sends `text` only on `text=1`. It sends at most 50 entries each of `inbound`, `reverse_links` and `term_uses`, with `inbound_count`, `reverse_count` and `term_use_count`; `inbound=all` sends them all. `inbound_near` lists the in-tree inbound sources when there are at most 16 (the Map's arcs), else `null`. The page fetches the rest when the reader asks for it (the `+N more` chip, the Term-uses disclosure, the stored Markdown, the writing desk). The MCP `get_card` answer does not change.
- **D9.** `scripts/gui-bench/cold-card.mjs` times the first Term-linked Card read after a write on a test box, and the next ping. It refuses port 4717.

## Slices

1. zt-check 0.19.2: linear term-table parse (D1).
2. Term meanings parsed once per list (D2).
3. A read lock for the Snapshot cache: single-flight build, stale reads during a build (D3).
4. A writer lane for GUI POSTs (D4).
5. `zt dump` without the edit lock (D5).
6. The page answers the latest move and shows a pending Card (D6).
7. Locks re-read only when they change, and a lean Card answer (D7, D8).
8. The cold-read benchmark, deploy and before/after measures (D9).

## Out of scope

- Carrying Term-list parses across Snapshots (proposal §5.3): measured first after slices 1–2.
- The Lock panel's fault recording under the edit lock: it is a write and stays one.
- MCP and CLI reads: they already read in their own read transaction and take no edit lock.
