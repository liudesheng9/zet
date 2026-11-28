---
block: coordinates
title: First-class multi-coordinate Cards and Connection trees
paths:
  - "src/coord.rs"
  - "src/connection.rs"
  - "src/coord_lock.rs"
  - "src/dependencies.rs"
  - "tests/connection_*.rs"
  - "tests/coordinate_*.rs"
  - "tests/topic_attachments.rs"
  - "tests/gui_coordinates.rs"
  - "tests/multi_coord_*.rs"
synced: 22a98736
updated: 2026-11-28
---

# First-class multi-coordinate Cards and Connection trees

Owner: Xingtong Chen

## Purpose

This block implements ZET's **First-Class Multi-Coordinate Card** architecture (docs/multi-coordinate-cards-proposal.md, building on docs/multi-tree-coordinates-proposal.md and docs/connection-trees-free-proposal.md). Every live Card has one immutable identity (`card_id`), one immutable `owner_workspace`, one commit-enforced **Filing Coordinate** (`filing_coord_id`, guaranteeing `>= 1` live Coordinate in `owner_workspace`), and **zero or more Secondary Coordinates** across any `Topic`, `Literature`, or `Connection` (`~n`) tree — including **multiple Coordinates in the same Tree** and **self-ancestry by `card_id`** (while `parent_coord_id` stays strictly acyclic).

It preserves every existing guarantee: one body per Card, Workspace authority, Single-Coordinate-Space paths (`MC4`: no path string means two Cards in one Tree), retired `+k` shelves (`MC1`), Tree Locks v2 with per-`coord_id` advisory drift & v1 Lock preservation, no silent Link rebinding (`[[…]]` cites canonical Filing addresses or Handles only; Secondary Coordinate paths are refused with canonical + `place:<coord_id>` hints), and byte-identical Model, Math, Term, Cited, and Hub reports.

**Owns:** Pure Schema v2 (`coordinates_schema_version = "2"`, Writer Epoch 2, `PRAGMA foreign_keys = ON`) and its unified tables (`trees`, `card_identity`, `card_handles`, `coordinates`, `coordinate_edges`, `retired_coordinates`, `applied_ops`), centralized transactional Card+Coordinate registration, unified Coordinate reads/writes (`connection::run`), the `MC4` `model`/`term` filing-chain splice guard, pure v2 Locks (`lock_coordinate_seals` `evidence_version = 2`, dual `OLD`/`NEW` SQL triggers, per-`coord_id` pins/drift/reviews), the `coord_id`-keyed Dependency Lens & seeding (`mode: "occurrences"`), the Session unified Coordinate pointer & verbs, the GUI `coord_id`-keyed Map/Crossings/repeat badges (`connections.js`), Dump v2 (`coordinates.json`), and Portable v4 export (`portable-rules.js`). All v1 structures and v1 → v2 migration code were removed after migrating `D:\zt` to v2.

**Does not own:** Card prose rules and Links ([Cards, Workspaces and Links](cards-and-links.md)), legacy Topic/Literature Location arithmetic ([Service, CLI and storage](service-and-cli.md)), legacy Lock catalog and seal (`src/tree_lock.rs`), and Model/Math build semantics ([Models and math](models-and-math.md)).

## At a glance

| | |
|---|---|
| Entry points | `connection::run(root, access, tool, args)` (MCP, GUI `POST /api/connection`, Session); `coord::{check_generation, ensure_schema, register_root_card_tx, register_regular_card_tx, register_regular_card_at_tx, register_connection_native_tx, sync_filing_moves_tx, check_model_term_splice_guard, check}`; `coord_lock::{snapshot, capture, host_locked, ensure_delete_allowed, ensure_moves_allowed, drift, review_source}`; `dependencies::{get, seed}` |
| Unified Coordinate MCP tools | `list_coordinates`, `get_tree_coordinates`, `add_coordinate`, `add_coordinates`, `move_coordinate`, `reorder_coordinates`, `detach_coordinate`, `refile_card`, `rebind_coordinate` |
| Connection & Lens MCP tools | `list_connection_trees`, `get_connection_tree`, `create_connection_tree`, `edit_connection_tree`, `create_connection_card`, `connect_placements`, `edit_connection`, `delete_connection`, `extract_branch`, `promote_card`, `get_dependencies`, `seed_connection_tree`, `review_source` (owner only); legacy adapters `place_card`, `list_placements`, `list_connections`, `list_side_mounts`, `edit_placement`, `move_placement`, `unplace`, `rebind_placement` |
| Tests | `tests/multi_coord_storage.rs`, `tests/multi_coord_ops.rs`, `tests/multi_coord_locks_and_coaction.rs`, `tests/multi_coord_surfaces.rs`, `tests/multi_coord_acceptance.rs` (all 8 release-blocking acceptance cases including 62,322-Card `D:\zt` temp-copy migration), plus `tests/connection_*.rs`, `tests/coordinate_*.rs`, `tests/topic_attachments.rs`, `tests/gui_coordinates.rs`, and `src/gui.rs` Node tests of `connections.js` |

## Concepts

- **Unified Coordinate table (Schema v2).** Every Topic, Literature, and Connection tree has one row in `trees(tree_id, role, workspace, public_key, display_num, root_card_id, ...)` and every occurrence of every Card (Filing or Secondary) has one row in `coordinates(coord_id, tree_id, host_workspace, card_id, parent_coord_id, edge, slot, gloss)`. There is **no** `UNIQUE(tree_id, card_id)` constraint; same-tree repeats and self-ancestry by `card_id` are first-class while `parent_coord_id` stays strictly acyclic.
- **Commit-enforced `>= 1` Filing floor.** `card_identity` stores `owner_workspace` and `filing_coord_id` with `CHECK((status = 'live' AND card_row IS NOT NULL AND filing_coord_id IS NOT NULL AND retired_at_rev IS NULL) OR ...)` and `FOREIGN KEY (filing_coord_id, card_id, owner_workspace) REFERENCES coordinates(coord_id, card_id, host_workspace) DEFERRABLE INITIALLY DEFERRED`. Every live Card is mathematically guaranteed at SQLite `COMMIT` to retain at least one live Coordinate in its `owner_workspace`.
- **Centralized Card+Coordinate registration.** The legacy `identity_after_insert` trigger is retired (`DROP TRIGGER IF EXISTS identity_after_insert`). Every Card creation flows through `coord::{register_root_card_tx, register_regular_card_tx, register_regular_card_at_tx, register_connection_native_tx}` inside a `SAVEPOINT` so `card_identity` + `coordinates` (+ `trees`/`card_handles`) satisfy deferred FKs atomically.
- **Sparse `slot` preservation.** Side branches store their explicit positive `slot` (`a=1, b=2, c=3, ...`) and derive Side labels from `slot`, preserving all legacy sparse sibling gaps (`303` sparse Side groups in `D:\zt`) with zero compaction and zero address rewrites.
- **Host admission (`MC1`, `MC3`, `MC4`).**
  - `MC1`: Legacy Topic `+k` shelves (`EdgeKind::Attach`) are retired; Topic trees admit Secondary Coordinates onto standard Direct/Side slots (`0/1|a`), sharing the Single-Coordinate-Space with Filing Coordinates.
  - `MC3`: Literature trees admit repeated Secondary Coordinates of their own filed Cards (including their root) and refuse foreign Cards.
  - `MC4`: Scholarly structures (`model`, `math`, `term`, `cited`, `hub`) read Filing Coordinates only; `check_model_term_splice_guard` refuses splicing a Secondary Coordinate directly between two active filing `model` or `term` parts; `LinkBody::parse` / `secondary_coordinate_at` refuses `[[…]]` targeting a Secondary Coordinate path with a hint naming the canonical Filing address and `place:<coord_id>`.
- **Projection (`crossings`) & `elsewhere`.** Keyed by `coord_id`. `elsewhere` lists every other Coordinate of a Card including same-tree siblings. `crossings` preserves parallel assertions, equal-Card/different-`coord_id` lines, Missing hops, and `exits_by_coord` without Cartesian explosion.
- **Locks v2.** `lock_coordinate_seals` enforces `evidence_version = 2` for every active Lock, sealing full `coordinates` + `coordinate_edges` + ordered lock-time pin baselines (`observed_authored_fp`) while allowing live external source text to drift (`review_source` accepts `coord_id`), guarded by dual `OLD`/`NEW` SQL triggers (`coord_lock_placement_*`, `coord_lock_edge_*`, `coord_lock_tree_*`, `coord_lock_filing_change`, `coord_pin_*`).

## Data and storage

- Tables: `trees`, `card_identity`, `card_handles`, `coordinates`, `coordinate_edges`, `retired_coordinates`, `applied_ops`, and Lock tables `lock_coordinate_seals`, `lock_placements`, `lock_connections`, `lock_placement_pins`, `lock_source_reviews`.
- `PRAGMA foreign_keys = ON` is enabled in both `initialize_database` and `open_db`.
- **Pure v2 generation gate:** `coord::check_generation` accepts only generation `2` (or fresh pre-coordinates backfill) and rejects legacy generation `1` or future generations without modifying the database. **Writer Epoch 2** (`WRITER_EPOCH = 2`) fences all tables against older writers.
- **Dump v2 & Portable v4:** `zt dp` writes `coordinates.json` version `2` (`trees`, `coordinates`, `coordinate_edges`, `retired_coordinates`, `card_identity`, `card_handles`); `zt export` writes Portable v4 for every unified tree (deduplicated `cards` preserving `owner_workspace`, full `coordinates` array, remapped `place:<coord_id>`).

## Testing

```bash
cargo test --test multi_coord_storage --test multi_coord_ops --test multi_coord_locks_and_coaction --test multi_coord_surfaces --test multi_coord_acceptance
cargo test --test connection_trees --test connection_locks --test connection_dependencies --test topic_attachments --test coordinate_storage --test connection_portability --test connection_session --test coordinate_review_fixes --test coordinate_third_review --test gui_coordinates
cargo test --bin zt gui::tests::the_connection
```

## Change history

- 2026-11-28: First-Class Multi-Coordinate Cards (`docs/multi-coordinate-cards-proposal.md`, `P1`–`P5`): Schema v2 (`coordinates_schema_version = "2"`, Writer Epoch 2, `PRAGMA foreign_keys = ON`), commit-enforced `filing_coord_id` floor, sparse `slot` preservation, atomic v1 → v2 migration with indexed bulk materialization (~10s on a 62,322-Card copy of `D:\zt`), `MC1` `+k` retirement, `MC3` Literature admission, `MC4` `model`/`term` splice guard & Secondary-Coordinate link refusal, 9 new Coordinate MCP tools, Locks v2 dual `OLD`/`NEW` triggers & per-`coord_id` drift review, GUI `coord_id` keying & `×N` badges, unified Session Coordinate verbs, Dump v2 & Portable v4.
- 2026-11-26: zt 0.70.0, free Connection trees (`docs/connection-trees-free-proposal.md`).
- 2026-11-24: zt 0.65.0–0.68.0, initial Multi-tree coordinates Phases 1–4.
