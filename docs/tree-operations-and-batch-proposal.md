# Fast tree operations, span operations and structural batches

Status: decided (2026-10-31), not built. PRD #228, slices #229–#237. The six open questions are settled in
[Decisions](#decisions-2026-10-31).
Scope: the structural writers of [Card operations](wiki/card-operations.md) (`move_card`,
`insert_card`, `delete_card`, a Side placement with `before`, a Citation key rename), the Link index
(`src/link_index.rs`), `apply_batch` (`src/batch.rs`), and the clients that drive them: agents
through `zt mcp`, scripts through `zt-digest call` and the model skills, and the owner through the
GUI, the Session and the shell.

## The problem

A tree operation is slow, and its cost grows with the size of the whole archive, not with the
number of Cards it changes. There is also no way to send several tree operations as one change, so
a reorganization of one model or one Topic is a long series of slow single calls.

### Measurements

The live archive (`D:\zt\zt.sqlite3`, read-only query, 2026-10-31) holds 5,151 Cards, 37,024 Link
rows and 8.2 MB of Card text in 12 trees.

The service log records an edit-lock hold only when it lasts more than 5 seconds
(`EDIT_HOLD_LOG_AFTER`, `src/main.rs:3117`) or when another writer waited. The current log
(`D:\zt\zt.log`) gives these numbers:

| Operation | Holds over 5 s | Mean hold of those | Longest hold |
|---|---|---|---|
| `move_card` | 242 | 8.6 s | 18.8 s |
| `apply_batch` | 1 | 12.6 s | 12.6 s |

| Operation that waited | Waits logged | Mean wait | Longest wait |
|---|---|---|---|
| `edit_card` | 12 | 8.8 s | 19.1 s |
| `move_card` | 5 | 5.5 s | 6.2 s |
| `apply_batch` | 3 | 6.3 s | 7.6 s |

An `edit_card` itself holds the lock for about 0.15 s. It waits up to 19 s because a move holds the
lock. A model fold or a settings pass that moves 30 Cards one by one therefore takes four to five
minutes of lock time, and every other writer (a digest job, the GUI) waits behind it.

### Where the time goes

One confirmed plain `move_card` (`ops::move_card`, `src/ops.rs:607`) with agent Access reads and
parses the whole archive six times:

| Step | Code | Whole-archive work |
|---|---|---|
| Find the moving subtree | `descendant_locations`, `src/main.rs:2903` | `load_cards` (all 8.2 MB of text) to get the Location set |
| Compute the mapping | `move_mapping`, `src/main.rs:4713` | `load_cards` again |
| Count the Link rewrites for the plan | `count_rewritten_links`, `src/main.rs:4764` | `load_cards`, then parse every Card body |
| Check that no human Card is rewritten | `Access::ensure_link_rewrites_allowed`, `src/workspace.rs:88` | `load_cards` again |
| Rename the Locations and rewrite Links | `apply_location_mapping_tx`, `src/main.rs:4527` | `load_cards`, then parse and rewrite every Card body |
| Regenerate Reverse links | `regenerate_reverse_links_tx`, `src/main.rs:4000`, into `link_index::rebuild`, `src/link_index.rs:130` | `load_cards`, parse every body, delete and insert all 37,024 Link rows, then `refresh_reverse` for all 5,151 Cards: one join query per Card, and one full text read and title parse per Link row |

The last step costs the most. It is a full re-index, and the Link index exists to avoid it:
ordinary edits already use the incremental path `link_index::sync_sources`
(`src/link_index.rs:173`, called from `src/batch.rs:898`). Only the structural writers still call
the full rebuild:

| Writer | Full rebuild at |
|---|---|
| Move | `apply_move_plan`, `src/main.rs:4670` |
| Insert, and a Side placement (`create_successor` with `before`, also inside `apply_batch`) | `place::apply_insert_plan_tx`, `src/place.rs:229` |
| Delete | `apply_delete_plan`, `src/main.rs:4416` |
| Citation key rename | `src/main.rs:3819` |

A batch that places 20 Term rows with `before` therefore runs 20 full re-indexes in one
transaction. That explains the 12.6 s `apply_batch` hold.

### Why many single calls make it worse

- `apply_batch` accepts only `create_topic`, `create_literature`, `create_successor` and
  `edit_card`. `move_card`, `insert_card` and `delete_card` are always single calls, each with its
  own lock, plan and full re-index.
- A client that previews first (`move_card` without `confirm`, as `zt-digest topic_join` does,
  `tools/zt-digest/src/topic_join.rs:584`) pays the planning passes twice.
- A sequence of single calls is not atomic. The `zt-model-fold` skill warns that "batched calls
  run even after one fails, and leave nested Model Cards", so it forbids a retype in the same
  sequence as moves and asks for one retype per message (`skills/zt-model-fold/SKILL.md:56`).
- A script plans from one read of the tree, but after each move the addresses change. Each script
  must therefore re-read or replay the rename map (`-Moves <log>` in the model skills) before the
  next call.
- Some common changes have no operation at all. To move a run of three Cards out of a chain, a
  client moves each Card `alone`; to reorder the Side children of a Card, it parks them and moves
  them back one by one (the fold recipe in `skills/zt-model-fold/SKILL.md:54`).

## Goals

1. The cost of a tree operation depends on the Cards it moves and their Link neighbours, not on
   the size of the archive.
2. Several tree operations can go to the server as one atomic change, with one plan, one
   confirmation, one lock and one refresh of the derived data.
3. A small set of higher-level tree operations covers the changes that agents, scripts and the
   owner now build from many single moves.
4. The same operation format works for every client: an MCP agent, a `zt-digest` script, the
   shell, the Session and the GUI.
5. Nothing weakens the current guarantees: Access, Tree Locks, the Write scope, the Card check,
   two-step confirmation of destructive changes, Links that follow every rename, Spreads that
   follow, receipts and replay.

## Part A: make each structural operation incremental

This part changes no tool, no argument and no reply. It changes only how the server does the work.

**A1. An incremental re-index for a Location mapping.** Add
`link_index::remap(conn, mapping, deleted)`, the structural counterpart of `sync_sources`:

1. Find the sources to rewrite from the index:
   `SELECT DISTINCT source FROM links WHERE target IN (mapping keys ∪ deleted)`.
   Only these bodies change. Rewrite them with the existing `replace_link_locations`.
2. Rename the moved addresses in the `links` table with two `UPDATE` statements over a temporary
   mapping table (one for `source`, one for `target`). Delete the rows of deleted sources.
3. Refresh the Reverse links only of the affected Cards: the moved Cards, the rewritten sources,
   and the targets of every Link whose source moved or was deleted. Use the existing
   `refresh_reverse` (`src/link_index.rs:224`).
4. Bump the archive revision once.

`apply_location_mapping_tx` then rewrites only the sources from step 1 instead of every Card, and
`regenerate_reverse_links_tx` is no longer called by the four writers in the table above.
`link_index::rebuild` stays for migrations and `zt reindex`.

**A2. Plan from the index, not from the Card text.**

| Today | Proposed |
|---|---|
| `descendant_locations` and `move_mapping` call `load_cards` | Both read the Location set once (`SELECT location FROM cards`, as `place::all_locations` already does) |
| `count_rewritten_links` parses every body | `SELECT COUNT(*) FROM links WHERE target IN (mapping keys)`. Each index row is one authored macro, Term uses included |
| `ensure_link_rewrites_allowed` parses every body | A join of `links` with `cards` that returns the first human-Workspace source whose Link targets a mapped address |

**A3. Read each source title once.** `refresh_reverse` reads and parses the full text of a source
once per Link row. Cache the parsed titles for the duration of one refresh.

**A4. The full re-index is the test oracle.** A test helper runs `link_index::check`
(`src/link_index.rs:286`) after every structural operation in the existing suites. A new property
test applies random moves, inserts, Side placements and deletes to a fixture, and compares the
incremental result to a full rebuild after each step. `zt reindex` remains the repair command.

**Expected effect.** A move of a subtree of k Cards with l Links in or out of it costs about
O(k + l) row operations, plus one read of the Location set. The target for a typical move on the
live archive is a lock hold under 0.2 s, against 5–19 s today. This is a target. Slice S1 measures
it on a box copy.

## Part B: structural items in `apply_batch`

**B1. New item tools.** `apply_batch` accepts `move_card`, `insert_card`, `delete_card`, and the
new operations of Part C as items, beside the existing creates and edits. The existing limits stay:
64 items and 1 MiB of authored text per real batch. A new limit caps the Cards that one batch
renames or deletes (proposed: 2,000).

**B2. Addresses name Cards, read on the batch's start state.** Every address in a structural batch
names a Card that exists when the batch starts, or a `@@client_ref` of a Card an earlier item makes.
The batch keeps one cumulative mapping of old to new Locations. Before each item runs, the batch
translates its addresses through that mapping, so an address follows its Card through every
earlier move. A script plans from one read and never computes intermediate addresses. An item that
names a Card an earlier item deleted is refused with `card_deleted_in_batch`.

An empty slot cannot follow a Card, so a structural batch names destinations by Cards only:

| Destination | Meaning |
|---|---|
| `after: X` | the next Direct successor of X (its current Direct successor and that chain move down) |
| `side_of: X` | the next free Side label of X |
| `before: X` | the place of X, which moves down (today's `before`) |

The single `move_card` call keeps `to` for compatibility. Inside a batch, `to` is refused.

**B3. One refresh at the end.** The items rename Locations and rewrite Link sources inside the
transaction. The batch collects the affected set of every item, then runs one `refresh_reverse`,
one `spread::follow` with the composed mapping, and one revision bump before the commit. This is
the same pattern that `sync_sources` already uses for edits.

**B4. One plan, one confirmation.** A batch that renames or deletes any Card needs confirmation:

1. `preview: true` simulates the batch (the existing savepoint simulation) and returns the composed
   plan: every `old -> new` Location, every deleted Card, every Link that would break, the count of
   rewritten Link macros, and a `plan_hash` (a hash of the archive revision and the canonical
   items).
2. The real call repeats the items with `confirm: <plan_hash>`. The server recomputes the hash. A
   different archive revision or a different item list is refused with `plan_changed`, and nothing
   is written.

A batch that only creates and edits needs no confirmation, as today.

**B5. Every current check still runs per item.** Access, the Link-rewrite gate, Tree Locks, the
Write scope (`scope::WriteScope::check`), the Card check on every new or edited Card, and receipts
with replay all apply to each item, on the state left by the items before it. A real batch stays
fail-fast and atomic. A preview reports refusals and blocked items as it does today.

**B6. What this removes.** The retype-and-move trap of the fold skill goes away: a fold becomes one
batch (retype, move, re-point, delete) that commits completely or not at all. The `-Moves` rename
logs become unnecessary, because the reply carries the composed mapping.

## Part C: new tree operations

Each operation computes one Location mapping (plus, for a delete, one delete set) and goes through
the same path as a move. Each one is a single MCP tool and a batch item, and each one is reachable
from the GUI, the Session and the shell. The list is short on purpose: the batch composes the
rest.

**C1. `move_span`: move a run of Cards.** A span is a run of consecutive Direct successors in one
chain, `from` through `through`, each with its Side successors. `move_span {from, through, after |
side_of | before}` moves the span to the destination and closes the gap: the Card after `through`
becomes the Direct successor of the Card before `from`. Today a client moves each Card of the span
`alone`, one slow call per Card. `move_card {alone: true}` becomes the span of one Card.

**C2. `delete_span`.** `delete_span {from, through, into?}` deletes a span and closes the gap in the
same way, with one plan and one confirmation. Without the span form, the current `delete_card`
deletes the Card and every later Direct successor.

**C3. `reorder`: give the order of a Card's children.** `reorder {parent, side: [X, Y, Z]}` puts the
listed Side children of `parent` in this order and keeps their subtrees. `reorder {parent, direct:
[...]}` does the same for the spans of a Direct chain. The server computes the permutation as one
mapping. The listed set must equal the current set of children. This replaces the park-and-return
pattern of the model skills and the shifts that a Term list needs.

**C4. `indent` and `outdent`: outliner moves.** Both act on a span (C1): by default the span of one
Card, that is the Card with its Side successors; `through` makes it longer. `indent {at, through?}`
makes the span the last Side branch of the Card before `at` in its chain. `outdent {at, through?}`
moves a span that starts a Side branch to `after` the branch's parent. In both, the gap closes as in
`move_span`. These are the moves that the Tab and Shift+Tab keys make in an outliner, where a
Card's children are its Side successors and its later Direct successors are its next siblings.

**C5. `relink`: re-point Links without a delete.** `relink {from, to, sources?}` rewrites every
authored Link to `from` (or only those in the listed `sources`) to point to `to`. Today only
`delete_card {into}` does this, so a fold must delete to re-point. The Access gate and the Tree
Locks apply to every rewritten source.

**Considered and left out.**

| Operation | Why it is left out |
|---|---|
| `swap` | `reorder` covers it |
| `merge` (append a body, re-point, delete) | A batch of `edit_card`, `relink` and `delete_card` does it atomically |
| Side-label compaction as its own call | A delete already compacts Side labels. It leaves a gap only when a broken Link would otherwise land on a renamed Card, and a later compaction would make that Link name the wrong Card |
| Copy a subtree | Cards never move or duplicate between Workspaces; a copy inside one Workspace makes two Cards with one idea |
| Bulk retype | `edit_card` items in a batch already do it; B5 makes them atomic with moves |

## Part D: one format for every client

The JSON item list of `apply_batch` becomes the one plan format.

| Client | How it sends a plan |
|---|---|
| MCP agent | `apply_batch` with `preview`, then with `confirm: <plan_hash>` |
| `zt-digest` and the model skills | `zt-digest call` with a plan file. The model skills replace their move loops with one batch per pass |
| Shell | New `zt apply <plan.json>`: it prints the composed plan, reads the confirmation word, then commits with the plan hash |
| Session | `mv` accepts a span (`mv 3/4..3/7 after 3/9`); new `ord`, `in` and `out` commands for `reorder`, `indent` and `outdent` |
| GUI | Multi-select in the tree view; drag of a selection; keys for indent, outdent and reorder. Each gesture builds the same items and shows the composed plan in one confirmation dialog |

## Decisions (2026-10-31)

The owner delegated the six open questions. They are settled as follows.

**D1. Start-state addresses that follow their Cards.** In a structural batch every address names a
Card on the batch's start state (or a `@@client_ref`), and the batch translates it through the
cumulative mapping before each item (B2). The other basis, the current address at each item, would
make every script recompute addresses after each move, which is the work this proposal removes.
Destinations name Cards (`after`, `side_of`, `before`); `to` is refused inside a batch.

**D2. A plan hash confirms a structural batch.** The preview returns `plan_hash`; the real call
passes `confirm: <plan_hash>` (B4). A hash binds the confirmation to the exact plan the client saw.
A fixed word (`move`, `delete`) would confirm whatever the batch does on the archive at commit time.
The single tools keep their words, so no existing client breaks.

**D3. Limits.** A real batch keeps 64 items and 1 MiB of authored text, and adds a cap of 2,000
renamed or deleted Cards. A larger reorganization is several batches. Tree operations go in `items` only:
`preview_items`, the 1,024-item planning mode for creates and edits, refuses them (settled while
building S3: following renames across its chunk boundaries buys nothing a client needs).

**D4. `indent` and `outdent` act on a span, by default the span of one Card.** The span of one Card
is the Card with its Side successors, which is what an outliner moves on Tab and Shift+Tab. Its
later Direct successors are its next siblings and stay; the gap closes. `through` makes the span
longer (C4). This replaces the `alone` flag for these two operations.

**D5. A span always moves with the Side successors of its Cards.** A Side successor is a thought
about the Card it branches from, so it stays with that Card. To leave a branch behind, a batch
moves it first (another item), then moves the span.

**D6. Owner surfaces in this order: shell, GUI, Session.** `zt apply` comes first, because it is
the plan format with the least code and it gives a tested owner path at once. The GUI comes next,
because the owner reorganizes trees there; its gestures are shown as mockups to the owner before
they are built. The Session commands come last.

## Delivery slices

Each slice is one GitHub issue under the PRD #228. Releases follow `docs/agents/workflow-principles.md`:
each zt release is built in `D:\zt-target`, deployed, and checked before the next slice that
depends on it. Timings are measured on a box copy of the archive, never on the live database.

| Slice | Issue | Content | Depends on |
|---|---|---|---|
| S1 | #229 | A1, A3, A4: incremental re-index for the four structural writers, the oracle tests, timings on a box copy | — |
| S2 | #230 | A2: planning and gates from the Link index | S1 |
| S3 | #231 | Part B: structural items in `apply_batch`, start-state addresses, one refresh, `plan_hash` confirmation, the 2,000-Card cap | S1 |
| S4 | #232 | C1, C2, C4: `move_span`, `delete_span`, `indent`, `outdent` as tools and batch items | S3 |
| S5 | #233 | C3, C5: `reorder` and `relink` as tools and batch items | S3 |
| S6 | #234 | `zt apply <plan.json>` in the shell | S3 |
| S7 | #235 | GUI: multi-select, drag of a selection, indent/outdent/reorder keys, one plan dialog (mockups first) | S4, S5 |
| S8 | #236 | Session: span syntax for `mv` and `del`, `ord`, `in`, `out` | S4, S5 |
| S9 | #237 | Clients: the model skills, `topic_join` and Organize send one structural batch per pass | S4, S5 |
