---
block: coordinates
title: Multi-tree coordinates and Connection trees
paths:
  - "src/coord.rs"
  - "src/connection.rs"
  - "src/coord_lock.rs"
  - "src/dependencies.rs"
  - "tests/connection_*.rs"
  - "tests/coordinate_*.rs"
  - "tests/topic_attachments.rs"
synced: 8b1f4345
updated: 2026-11-26
---

# Multi-tree coordinates and Connection trees

## Purpose

This block lets one Card stand in several trees without being copied: one canonical Home, at most one coordinate per tree, and an identifiable author for every claimed relationship (docs/multi-tree-coordinates-proposal.md). It adds a third tree role, the **Connection tree** `~n` (关联树), in which Cards from anywhere are mounted, arranged on a Folgezettel spine, written natively (`~n#k`) and connected by **tree connections**; **Topic-side mounts** `<loc>+k` beside Topic Cards; the cross-book **Dependency Lens**; and one-time seeding. Since zt 0.70.0 (docs/connection-trees-free-proposal.md) a Connection tree is fully free: its root has no meaning and may stay untitled, any set of Cards mounts at once, and every Topic or Literature tree reads the **projection** of the Connection trees that cross it. It keeps every existing guarantee: one body per Card, Workspace authority, Tree Locks and pins, no silent Link rebinding, and byte-identical Model, Math, Term, Cited and Hub reports.

**Owns:** the coordinate storage (`card_identity`, `card_handles`, `connection_trees`, `placements`, `placement_edges`, `retired_placements`), the identity triggers and the writer fence, the address grammar of handles and coordinates, tree shapes and coordinates, every Connection-tree and Topic-side mount operation and read, the coordinate Lock layers (`lock_coordinate_seals`, `lock_placements`, `lock_connections`, `lock_placement_pins`, `lock_source_reviews`), the Dependency Lens and seeding, the Session's station pointer and verbs, the GUI's Connection-tree rules (`connections.js`) and views, the dump's `coordinates.json` and the version-3 portable export of a Connection tree.

**Does not own:** Card text, Links and their index ([Cards, Workspaces and Links](cards-and-links.md)), the legacy Topic and Literature Location rules and the delete planner ([Service, CLI and storage](service-and-cli.md)), the legacy Lock catalog and seal ([Service, CLI and storage](service-and-cli.md), `src/tree_lock.rs`), Model and Math structure ([Models and math](models-and-math.md)).

## At a glance

| | |
|---|---|
| Entry points | `connection::run(root, access, tool, args)` (every MCP tool below, the GUI's `POST /api/connection`, the Session's station verbs); `coord::ensure_schema`, `coord::register_sql_functions`; `coord_lock::{snapshot, capture, host_locked, ensure_delete_allowed, ensure_moves_allowed, drift, review_source}`; `dependencies::{get, seed}` |
| MCP tools | reads `list_connection_trees`, `get_connection_tree`, `list_placements`, `list_connections`, `list_side_mounts`, `get_dependencies`; writes `create_connection_tree`, `edit_connection_tree`, `place_card` (also `edge: attach`), `create_connection_card`, `edit_placement`, `move_placement`, `unplace`, `rebind_placement`, `connect_placements`, `edit_connection`, `delete_connection`, `extract_branch`, `promote_card`, `seed_connection_tree`, `review_source` (owner only) |
| Tests | `tests/connection_free.rs` (0.70.0: untitled roots, block mounts, sort and filter, the projection, the delete plan hash, one-pass tree counts, export Workspaces), `tests/connection_trees.rs` (Phase 1 and the continuous acceptance path), `tests/connection_locks.rs` (Locks, pins, drift, review, promotion, native Events), `tests/connection_dependencies.rs` (lens, overlay, seeding), `tests/topic_attachments.rs` (Phase 4 invariance), `tests/coordinate_storage.rs` (migration, writer fence, identity, specialized-type invariance), `tests/connection_portability.rs` (dump round trip, export), `tests/connection_session.rs` (Session), `tests/coordinate_review_fixes.rs` and `tests/coordinate_third_review.rs` (review regressions); `src/gui.rs` unit tests of `connections.js` |

## Concepts

- **Identity.** `card_identity` holds one row per Card ever created. SQL triggers on `cards` insert a live identity on every insert, update its last address and title on every address or title change, and retire it (never delete it) on every delete, so every existing write path (create, edit, move, insert, compaction, span operations, citation-key rename, batch, `del --all`) keeps it in step without code at the call site. A retired identity is never revived; a new Card at a reused Location gets a new identity.
- **Handles.** `~n` (a Connection root) and `~n#k` (a native Card). A native Card's stored `location` is its handle; after `promote_card` its `location` is a Topic Location and `card_handles` keeps the handle pointing at it. The Link lifecycle, the Link index's reverse refresh, `load_card`, `get_card(s)` and Broken-link reads resolve a promoted handle through `coord::aliases`/`coord::canonical`; `links.target` stays literal, so a later move of `3/6` rewrites only literal `[[3/6]]`.
- **Roles.** `TreeRole::{Topic, Literature, Connection}` follows from the stored address (a partition: digit, letter, `~`). `tree_id("~7#4")` is `~7`, `is_tree_member` and `final_tree_root` know Connection trees, `ops::kind` answers `connection` for a root and `ops::zone` `connection` for its members.
- **A root without meaning (D2).** `create_connection_tree`, `seed_connection_tree` and `extract_branch` take an optional title. A root written without one is stored with its own handle as its title (`# ~7`): a Card title is never empty, and a title equal to the root's own handle carries no meaning. `connection::root_title` reads such a root as `untitled` (and errors on an unreadable root, never calling it untitled); `tree_summary` answers `title: ""` and `untitled: true`. Raw Card reads, edit bases, fingerprints and exported Card text keep the stored `# ~7`; only the views (cabinet, desk, Map root, reader title, Lock card, Link label) show "(untitled)" or the address alone.
- **Projection (D4).** `connection::tree_coordinates(conn, tree)` answers both coordinate views of one Topic or Literature tree from one membership read (`plane_placements`: the placements whose live Card has its home in the tree, by the same index range as before) and one shape per Connection tree (`host_shapes`): `elsewhere` (the stamps) and `crossings` (the projection). For each Connection tree with a station in the plane, `crossings` names those stations, a structure line from each to its nearest in-plane ancestor through `parent_id` (Direct and Side alike; `hops` counts the stations outside the plane in between, `edge` is the kind of the step out of the ancestor), every tree connection whose two stations are in the plane (each parallel assertion its own line, with both placement IDs), and the exits of each in-plane station (its structural neighbours and connections outside the plane). The root station is the tree itself: it is in no plane, ends the climb and never counts as an exit, by structure or by connection. A Missing mount is outside every plane but still a hop. Topic-side mounts are not part of the projection.
- **Coordinates.** Derived, never stored: `~7/0` is the root, the root's Direct chain is `~7/1`, `~7/2`, …, a Side branch is `|a`, `|b` with the legacy arithmetic; `3/2+1` is the first mount beside `3/2`. `LinkBody::parse` refuses a coordinate or `place:` inside `[[…]]` with a hint.

## Architecture

- `src/coord.rs`: the schema, identity triggers and writer fence; `TreeRole`, `Handle`, `Coordinate`; the resolver (`canonical`, `aliases`, `spellings`); tree shapes and coordinates; delete hosts and impact lines; Missing mounts; the dump's `coordinates.json`; `check`.
- `src/connection.rs`: `run` (one read transaction for reads; one Immediate transaction, receipts and one revision bump for writes; `preview` never writes), `confirm_plan`, every write and read operation.
- `src/coord_lock.rs`: the coordinate Lock layers and their triggers, pins, source fingerprints, drift, owner reviews, and the faults the Tree Lock scan reports.
- `src/dependencies.rs`: the Dependency Lens (`get`) and seeding (`seed`).
- Outside this block's paths: the Session's station pointer and verbs (`src/session/station.rs`, [Terminal Session](session.md)) and the GUI's Connection-tree rules (`src/gui/connections.js`, [GUI](gui.md)) call `connection::run`.

## Interfaces

The MCP tools in the table above take and return JSON. A write that changes what a reader saw answers a plan first (Behaviour). The GUI posts the same requests to `POST /api/connection` as the owner; the Session's station verbs call `connection::run` with owner Access. Links name a Connection root as `[[~7]]` and a native Card as `[[~7#4]]`; a coordinate inside `[[…]]` is refused.

## Behaviour

- **Block mounts (D1).** `place_card` with `targets` (`place_many`): the batch is resolved against the tree as it stands, deduplicated by Card identity in first-occurrence order, and spliced in as one block: with a Direct edge a chain after the parent (`P → A → B → old`, only `old` changing predecessor), with a Side edge consecutive Side branches before `before` or appended. One transaction, one gloss review (the plan's effects carry `order` and `displaced`), one revision. Cards already in the tree are reported in `already_placed`; a batch that places nothing is a successful no-op whose `op_key` receipt is kept. A bad target refuses the whole batch; `gloss` and `edge: attach` are refused with `targets`; at most 500 Cards.
- **Listing (D3).** `list_connection_trees` takes `sort` (`number`, the default, or `recent`: the highest handle first, because handles are never reused) and filters on the handle, the root's title and the purpose before any count is made.
- **Plans.** A write that changes what a reader saw (move, unplace, rebind, delete connection, extract, promote, seed, a splice that displaces a gloss) answers `{applied: false, confirm, lines, effects, plan_hash, expected_revision}` first. A confirmation must carry the word; a passed `expected_revision` or `plan_hash` that no longer matches is refused as stale. `op_key` stores a receipt in `applied_ops`.
- **Gloss review.** A station whose structural predecessor changes and has a gloss must be named in `gloss_updates` (a string keeps or replaces it, null clears it). A client that started a gloss review plans again with the decisions as a `preview` and applies only with the confirmation word; a plan that no longer asks for one changed under the review, and nothing is written.
- **Missing mounts.** A home delete retires the identity; stations in other trees keep their row, gloss, children and connections and read as `status: missing`. `list_broken_links` lists them apart (`missing_mounts`). A deleted Connection root takes its tree (stations, connections, record) and its still-native Cards; a deleted Topic anchor takes the mounts beside it.
- **Locks.** Locking `~7` makes its root and native Cards Lock members (the legacy seal, triggers and pins) and captures the coordinate evidence: frozen stations and connections (SQL triggers refuse any change of a locked host's rows), pins on live mounted targets (`pinned-delete`, `pinned-move`), the authored fingerprint of each mounted source at Lock time (advisory drift), and append-only reviews (`review_source`, owner only). A Topic Lock freezes its side mounts, also a Lock made before mounts existed. The seal and `reseal` are never touched.
- **Write scope.** A Literature-scoped `zt mcp` connection runs no Connection-tree tool and names no `~` Card. Its Link rewrites are checked on the real mapping: `scope::ensure_no_connection_rewrite` runs while `WriteScope::rewrite_guard` is alive, in `Access::ensure_link_rewrites_allowed` (plan time) and in the one re-keying point `apply_location_mapping_tx` and the delete repoint (apply time), so a compaction that renumbers a cited sibling is refused and a metadata edit that keeps the Citation key is not. A Link by handle is never rewritten.
- **Integrity.** `coord::check` (shown as the `coordinates:` line of `zt status`) reports Cards without identity, identities without Card, foreign-key faults of the coordinate tables, tree shapes, native Cards without a home station and Topic shelves whose ordinals are not 1..N.

## Data and storage

Tables: `card_identity`, `card_handles`, `connection_trees`, `placements`, `placement_edges`, `retired_placements`, `applied_ops`, and the Lock layers `lock_coordinate_seals`, `lock_placements`, `lock_connections`, `lock_placement_pins`, `lock_source_reviews`. `coord::ensure_schema` installs them, backfills identities for existing Cards and is idempotent; `initialize_database` calls it on every open.

- **Writer fence.** Every table carries `writer_epoch_*` triggers that call `zt_writer_epoch()`, which only current binaries register (`tree_lock::register_sql_functions` registers it with the coordinate functions). An older `zt`, or plain SQLite, can read the archive but every write it attempts fails. Tests that seed archives with raw SQL use `tests/support/writer.rs`.

## Configuration

None. Phase 5 has no switch: it is gated in code (Decisions and pitfalls).

## Usage

```text
create_connection_tree {title}                      -> ~1
place_card {host: "~1", parent_coord: "~1/0", target_at: "Smith2024/3"}
create_connection_card {tree: "~1", parent_coord: "~1/1", title, body}   -> ~1#1
connect_placements {tree: "~1", from_placement_id: "~1/2", to_placement_id: "~1/1", direction: "directed", kind: "rests-on"}
get_dependencies {at: "Smith2024/3|a", scope: "unit", overlay_tree: "~1"}
zt export ~1 --out <folder>
```

## Testing

Run `cargo test --test connection_trees --test connection_locks --test connection_dependencies --test topic_attachments --test coordinate_storage --test connection_portability --test connection_session --test coordinate_review_fixes --test coordinate_third_review`, and `cargo test --bin zt gui::tests::the_connection` for the page rules (needs Node).

## Decisions and pitfalls

### Phases

Phases 1–4 of the proposal are implemented and tested. Phase 5 (structured native Models and Math units in Connection trees with a `home_structure` table) is gated: a Connection tree authors `idea`, `event` and `finding` Cards natively and refuses other types with `native_structure_required`; existing Model, Math, Term, Cited and Hub Cards can be mounted and connected freely.

### Deviations and accepted limits

The deviations from the proposal's §6.2 and the accepted limits are listed in docs/multi-tree-coordinates-proposal.md §12. A Literature-scoped rewrite check must read the writer's real mapping (a compaction renumbers uncited siblings); an over-approximation from the named subtree both misses those siblings and refuses harmless edits.

## Change history

- 2026-11-26: zt 0.70.0, free Connection trees (docs/connection-trees-free-proposal.md, two Astra rounds): an optional root title read as `untitled`, `place_card` `targets`, `list_connection_trees` `sort` and handle filter, `tree_coordinates`/`crossings` (the projection) with one shape per host (before, `elsewhere` rebuilt a host's shape once per placement), and the review fixes: a delete confirmation bound to its plan and to the state behind it (`ops::delete_reviewed`, `plan_hash`, `coord::delete_state`: deleted texts, the stations and connections the delete removes or leaves missing), an all-skipped `targets` batch that checks a passed confirmation, an optional `extract_branch` title, `Snapshot::trees` in one pass, the portable v3 Workspace of a mounted Card, a promoted handle's live Link title.

- 2026-11-24: zt 0.67.0: `connection::elsewhere` names, for one Topic or Literature tree, every coordinate its Cards have outside it; the GUI stamps them on each Card's head ([GUI](gui.md)).
- 2026-11-24: zt 0.66.2: a portal station counts its tree's Cards with two index lookups (`home_tree_size`), not a `LIKE` scan.
- 2026-11-24: zt 0.66.0: the GUI draws a Connection tree with the card Map, each station on its home tree's colour ([GUI](gui.md)).
- 2026-11-24: the block (zt 0.65.0): Phases 1–4, four Astra review rounds with regression tests (`tests/coordinate_review_fixes.rs`, `tests/coordinate_third_review.rs`).
