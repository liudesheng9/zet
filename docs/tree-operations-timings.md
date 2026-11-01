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
item still refreshes its own Reverse links. S3 (#231) refreshes once per batch.
