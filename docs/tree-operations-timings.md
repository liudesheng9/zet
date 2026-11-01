# Tree operation timings

The timings that PRD #228 (`docs/tree-operations-and-batch-proposal.md`) asks each slice to record.
Every run uses a fresh **box copy** of the live archive (`VACUUM INTO` from `D:\zt\zt.sqlite3`, read
only), a private copy of the measured `zt.exe`, its own `ZT_CONFIG_DIR` and service
(`ZT_GUI_PORT=0`). The live database is never written. A run measures the wall time of one
`tools/call` over a fresh `zt mcp` stdio session, which is the edit-lock hold plus a few
milliseconds of process overhead (`get_card` shows that overhead). The harness is
`scripts/tree-ops-bench.mjs` (`node scripts/tree-ops-bench.mjs --zt <zt.exe> --label <name>`).

The copy holds 5,135 agent Cards (2026-10-31). The Cards the harness picks:

| Role | Address |
|---|---|
| Card with the Side successors `a`, `b` and `c` | `grossman2002integration/2` |
| `alone` pair (two Direct successors, no Side successors) | `acemoglu2025nobel/1\|b\|1` and its Direct successor |
| Leaf for create and delete | `ebrill2001modern/26\|b\|1` |

## S1 (#229) and S2 (#230): zt 0.50.0 against zt 0.50.1

Both builds are release builds. Milliseconds per call; `/2` is `grossman2002integration/2`.

| Call | 0.50.0 | 0.50.1 |
|---|---|---|
| `get_card` (overhead) | 7–8 | 7–8 |
| `move_card` plan, `/2\|b` before `/2\|a` (no `confirm`) | 5,190–5,481 | 49 |
| `move_card` subtree, `/2\|c` before `/2\|a`, confirmed | 11,615 | 130 |
| `move_card` `alone`, confirmed (two runs) | 1,944–1,964 | 69–71 |
| `insert_card` before `/2\|b`, confirmed | 2,136 | 72 |
| `create_successor` (Side, at the leaf) | 81 | 30 |
| `delete_card` of that new leaf, confirmed | 1,236 | 31 |
| `apply_batch` of 20 Side placements before `/2\|a` | 265,302 | 6,045 |

Where the time went in 0.50.0:

- every structural writer rebuilt the whole Link index and refreshed every Card's Reverse links
  (fixed in S1: `link_index::Changes`);
- the planners and the agent's Link-rewrite gate read and parsed every Card (fixed in S2);
- the Lock gate's `check_mapping` read the whole archive once **per mapped Card**, and its
  `Delete` and `LinkRewrites` checks once per call; a plan and its apply both ran it (fixed in
  S2: rows by address, sources from the index).

The Side placement batch still costs about 0.3 s per item in 0.50.1: each placement before `/2|a`
renames every later Side branch of `grossman2002integration/2` with its whole subtree, and each
item still refreshes its own Reverse links. S3 (#231) refreshes tree operations once per batch; a
Side placement keeps its own refresh (see below).

## S3 (#231) to S5 (#233): tree-operation batches in zt 0.51.0

Release build, a fresh box copy. Milliseconds per call. The 20 moves each move one leaf of
`acemoglu2010labor` to `side_of` that tree's `/1`, named on the batch's start state.

| Call | zt 0.51.0 |
|---|---|
| `move_card` plan, `/2\|b` before `/2\|a` | 44–45 |
| `move_card` subtree, confirmed | 143 |
| `move_card` `alone`, confirmed | 63–65 |
| `insert_card`, confirmed | 69 |
| `delete_card` of a leaf, confirmed | 18 |
| `apply_batch` of 1 move: preview / commit | 29 / 40 |
| `apply_batch` of 20 moves: preview / commit | 190 / 206 |
| `apply_batch` of 20 Side placements before `/2\|a` | 6,908 |

Two costs found while timing the batch, both fixed in 0.51.0: `crate::addresses` read every row of
`cards` (text pages included) through `COALESCE`, about 9 ms a call and several calls per plan; it now
reads the two UNIQUE indexes. `link_index::Changes::renamed` cleared its rename marks with two scans
of the whole `links` table per rename; it now clears them row by row through the indexes. Before the
fixes a 20-move batch took 932 ms.

Against the S3 target: a 20-move batch (206 ms) is about 6 times faster than 20 single moves (20 ×
63–143 ms plus 20 round trips), but not under twice one single move (126 ms). Each item still plans
on its own (about 8 ms: the Location set, the Link index and the Lock state); what a batch saves is
one lock, one Reverse-link refresh and one round trip per item. A Side placement (`create_successor`
with `before`) still refreshes per item, because the digests also send it in batches without tree
operations; each placement before `/2|a` renames every later Side branch of that Card.
