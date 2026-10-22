# Tree Lock release qualification (issue #141)

This is the release package for **S18 / #141**, the qualification slice of [PRD #123](https://github.com/liudesheng9/zet/issues/123). It records what the delivered Tree Lock code does at its public seams, which suite proves each named acceptance scenario and delivery group, which documentation contract now carries it, and which scopes stay open. It applies no Lock to real owner content.

Source documents: the [implementation design](tree-lock-implementation-design.md) (SHA-256 `fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a`), the [issue map](tree-lock-issue-map.md) and the [proposal](tree-lock-proposal.md).

**Delivered and verified here.** The six delivery groups of design §17 (storage and gate, GUI, Session/status/dump, joined delete, MCP and agent integrations, workflows) as one release: the root all-target suite, the two tool crates, the digest PowerShell/Node harness, the frozen Workflow `assemble verify` checks, the browser-side JavaScript checks (run by the crate's own unit tests), the developer wiki, and the installed binary on the live service with Link and Lock integrity.

**Not claimed here.** No real book digest or digest stage ran; the digest Workflow itself is not executed (the user runs it with Claude Code). No real owner tree is locked, emptied or migrated, and no deliberate token/SQL bypass or external source-document mutation is inside the promise (§8).

## 1. Release identity

| Item | Value |
|---|---|
| Branch and commit | `main` at the commit that carries this file, 0 ahead / 0 behind `origin/main` before the commit |
| `zt` | `zt 0.26.0`, Rust 2024, SQLite bundled |
| `zt-check` | `0.1.0` (`tools/zt-check`) |
| `zt-digest` | `0.9.0` (`tools/zt-digest`, takes `zt-check` as a library, embeds the current blueprint versions) |
| Frozen workflows | `zt-econ-digest` 5.11.0, `zt-math-digest` 4.11.0, `zt-idea-grow` 2.3.0, `zt-issue-dev` 1.3.0 — `assemble.mjs verify --scope project` reports `ok: true`, `problems: []`, `libraryDrift: []` for all four |
| Installed binaries | rebuilt from this commit and installed beside the config (`~/.cargo/bin`); the previous copies are kept beside them as `*.old-<time>` / `*.held-<time>` |

**Version note.** The design's §19 target was `0.22.0`, the version the first slice shipped (`S01 / #124` bumped `0.21.0 → 0.22.0`). Every later slice that changed user-visible behaviour bumped its own minor (`#132` → 0.23.0, `#133` → 0.24.0, `#135` → 0.25.0, `#136` → 0.26.0); the slices after `#136` changed workflows, digests and documentation without a version bump. This release therefore identifies itself as **0.26.0**, which contains every Tree Lock slice. The installed binary reports `zt 0.26.0` and the daemon record says `version=0.26.0`.

## 2. Suites run (R399–R402 / S18-AC02)

All commands ran on Windows from `E:\zet`. The root suite and the two tool crates use the repository's separate target folders (`E:` has little free space, and the live service keeps its own build). The digest harness runs against the installed release binary so that `zt`, `zt-check` and `zt-digest` all resolve to the current release (a build folder passed as `-Zt` must not contain a stale `zt-digest.exe`, see §10).

| Check | Command | Result |
|---|---|---|
| Root, all targets, locked | `CARGO_TARGET_DIR=D:\zt-target cargo test --all-targets --locked` | **679 passed, 0 failed, 1 ignored** (the ignored test is the suite's own manual real-clipboard smoke test) — log `D:\zt-target\qualification\issue141\root-tests-final.log` |
| Checker, locked | `CARGO_TARGET_DIR=D:\zt-check-target cargo test --manifest-path tools/zt-check/Cargo.toml --locked` | **23 passed, 0 failed** |
| Digest tools, locked | `CARGO_TARGET_DIR=D:\zt-digest-target cargo test --manifest-path tools/zt-digest/Cargo.toml --locked` | **343 passed, 0 failed** |
| Aggregate public runner | `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 4 -Zt C:\Users\chenx\.cargo\bin\zt.exe -Logs D:\zt-target\qualification\issue141\public-final` | **91 suites, 0 failed, 403 s**, 3 501 named `ok:` assertions; logs in `D:\zt-target\qualification\issue141\public-final` |
| Frozen framework harnesses | inside the aggregate runner (`framework-harness.mjs`, `bridge-workflow-harness.mjs`, `grow-workflow-harness.mjs`) | pass |
| Framework protocol own tests | `node ~/.claude/skills/workflow-framework-protocol/tests/run-tests.mjs` | **28 passed, 0 failed** |
| Workflow assembly | `node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs verify <name> --scope project --project E:\zet` | `ok: true`, `problems: []`, `libraryDrift: []` for the four frozen workflows |
| GUI JavaScript checks | `cargo test --bin zt gui::` (`src/gui.rs` `tests` run `src/gui/draft.js`, `src/gui/ask.js`, `src/gui/lock.js` and the whole page under Node) | **55 passed, 0 failed**, including `the_page_scripts_parse_as_javascript` |
| Developer wiki | `node ~/.claude/skills/project-wiki/scripts/wiki.mjs check` | the wiki is current, 17 blocks fresh |

The aggregate runner discovers every `*.tests.ps1` and `*harness.mjs` under `scripts/book-digest/tests/`; nothing is registered anywhere else. The digest **Workflow** is not run here; only its tools, its frozen assembly and its simulated modes are verified.

## 3. Named acceptance scenarios (R354 / S18-AC01)

Every scenario of design §18.1 and §18.2 has a passing assertion at its public seam. "Suite" names the test; the quoted text is the behaviour it proves, not only the test name. All of these ran in the root suite above.

### 3.1 Lock scenarios T01–T24

| ID | Suite and evidence |
|---|---|
| T01 | `tests/tree_lock.rs::an_owner_lock_freezes_a_topic_tree_for_every_writer` (a populated Topic tree, Owner and Agent alike); `a_literature_tree_and_a_root_only_tree_anchor_locks` (a Literature tree and a root-only tree anchor; a Regular Card anchor is refused); unit `a_regular_card_and_a_second_lock_are_refused` |
| T02 | `every_public_writer_path_is_refused_before_it_writes` (title, body, owned paragraph, type, facet, emphasis, level, Hub level, Math form, BibTeX through MCP, GUI and shell; the complete stored state is unchanged); `a_literature_tree_and_a_root_only_tree_anchor_locks` (a BibTeX edit without a rename is an edit refusal, a rename is a move refusal) |
| T03 | `every_public_writer_path_is_refused_before_it_writes` (direct successor at the last main-line Card, side successor, insert; same Card count, revision, Topic allocation, receipts, Links and Lock baseline afterwards) |
| T04 | `a_move_into_the_tree_and_a_delete_that_repoints_a_pin_are_refused` (moving an unlocked subtree into the Locked tree is `move-inside` from the complete final plan; no `__moving__` row survives); `every_public_writer_path_is_refused_before_it_writes` (insert and move of a member); unit `a_refused_mapping_writes_no_temporary_location` |
| T05 | `a_pinned_target_stays_editable_without_renaming_it` (body, title, type, facet, emphasis, level, form and BibTeX stay editable; the Citation-key rename is refused; seal unchanged); `an_unlocked_pinned_hub_stays_editable_and_only_its_address_is_frozen` |
| T06 | `an_owner_lock_freezes_a_topic_tree_for_every_writer` (a pinned target's move and delete are refused and name the source); `a_pinned_target_stays_editable_without_renaming_it` (rename); `a_move_into_the_tree_and_a_delete_that_repoints_a_pin_are_refused` (delete with `into` would rewrite a member's authored Links); unit `moving_an_ancestor_of_a_pinned_target_is_refused`, `a_delete_plan_is_refused_when_it_names_a_member_or_a_pinned_target` |
| T07 | `two_locks_pin_one_target_and_release_one_at_a_time` |
| T08 | `an_incoming_link_and_a_reply_keep_a_locked_tree_untouched` (an added and a removed incoming Link, and a Reply to a locked agent Card; every authored fingerprint and the seal unchanged, the Link index still exact) |
| T09 | `a_draft_into_a_locked_destination_leaves_no_partial_card` (Reply and hub/Spread filing refused atomically, the draft kept, no fallback Topic or partial Card) |
| T10 | unit `a_raw_registered_connection_cannot_bypass_the_gate_and_its_abort_rolls_back` (a registered connection's SQL meets the trigger, and the enclosing transaction rolls back) |
| T11 | unit `a_connection_without_the_sql_wrappers_reads_but_cannot_write_a_locked_card` |
| T12 | unit `a_locked_topic_tree_does_not_confuse_a_neighbouring_tree_id` (tree-id prefix and case boundaries), `zt_tree_root_resolves_topic_literature_and_moving_placeholders`, `a_refused_mapping_writes_no_temporary_location` (a temporarily detached parent leaves no `__moving__` row); the public append-at-main-line case is in T03 |
| T13 | `a_stale_unlock_dialog_cannot_release_a_later_lock`; `a_lock_plan_recomputes_every_fact_the_browser_sends` (the server ignores a browser's own counts and seal); `a_stale_confirmation_never_commits_and_a_lost_answer_is_read_back` (`plan_changed` on a changed revision) |
| T14 | `a_committed_receipt_replays_after_the_owner_locks_its_result` (the replay performs no Card write; a new key for the same edit is refused) |
| T15 | `a_batch_with_a_locked_item_rolls_back_every_unseen_fact` (earlier valid writes, receipts, allocation, Link rows and revision roll back together) |
| T16 | `a_preview_reports_every_independent_locked_item_and_blocks_its_dependents`; `a_refused_create_blocks_every_dependent_level_and_invents_no_address` |
| T17 | `every_read_projection_names_the_same_lock_facts_at_one_revision` (`get_card`, both `get_cards` views, `get_tree` with and without bodies, `list_trees`, `search_cards` in both modes, `export_titles`, and the GUI's `/api/overview`, `/api/tree`, `/api/graph` and `/api/card` against the stored `locks`/`lock_pins` rows); `every_read_path_still_serves_locked_cards` |
| T18 | `every_read_projection_names_the_same_lock_facts_at_one_revision` (a note-only change writes no Card, moves the overview fingerprint and advances the revision once); unit `a_note_change_advances_the_revision_only_when_it_changes` |
| T19 | `a_broken_link_is_a_warning_that_a_relock_captures` (the baseline warning is retained, pins nothing, and joins the promise only after unlock and relock) |
| T20 | `the_panel_records_a_new_integrity_fault_once`; `an_orphaned_faulted_lock_is_reported_and_released_from_the_panel`; units `check_reports_a_changed_member_and_a_dropped_trigger_once`, `extra_missing_and_moved_members_are_distinct_faults`, `every_pin_and_link_disagreement_is_its_own_fault`, `a_later_clean_comparison_does_not_erase_a_latched_fault` |
| T21 | `a_service_start_records_a_lost_trigger_before_repairing_it`; units `a_trigger_lost_before_startup_is_recorded_before_it_is_reinstalled`, `all_six_canonical_triggers_are_compared_and_restored`; `an_authorized_card_rebuild_keeps_the_promise_and_its_protection` |
| T22 | `a_failed_lock_note_or_unlock_leaves_no_partial_record` (a fixture-only trigger makes the history insert fail; no Lock, member, pin or event row and no revision survive, and the retry commits once); `a_stale_confirmation_never_commits_and_a_lost_answer_is_read_back`; `a_stale_unlock_dialog_cannot_release_a_later_lock`; unit `a_failed_install_keeps_the_prior_archive_retryable_and_its_backup` |
| T23 | `an_orphaned_faulted_lock_is_reported_and_released_from_the_panel` (the faulted record is released by id after its root is gone, the old history stays); `unlock_releases_exactly_one_lock_and_keeps_its_history` |
| T24 | `a_dump_carries_the_active_lock_member_pin_and_integrity_evidence` (active); `a_dump_keeps_an_ended_lock_and_a_deleted_root_as_inspectable_history` (ended); `a_dump_exports_lock_faults_as_evidence_and_records_nothing` (faulted); `dump_preserves_card_text_hash_mapping_and_location_order` (`locks.json` with version 1, the revision, empty `locks`/`events` and `integrity ok` on a no-Lock archive); units `dump_export_orders_several_locks_their_events_and_their_pins_deterministically`, `dump_export_answers_version_one_with_empty_evidence_at_the_archive_identity` |

### 3.2 Delete scenarios D01–D12

| ID | Suite and evidence |
|---|---|
| D01 | `tests/cli_service.rs::delete_takes_exactly_one_selector_and_refuses_every_other_form` (both or neither selector, a repeated selector or flag, an unknown argument and `--keep-root` beside `--all` are refused with the one usage line, before the service, a plan or a prompt) |
| D02 | `delete_confirmations_refuse_a_wrong_word_or_eof_for_every_selection` (all four selections: `delete`, the root address, the root address with keep-root, `delete all`; a wrong word and EOF cancel with no Card, Link or revision change, and the exact word applies exactly that selection); `keep_root_refuses_a_regular_card_and_its_cancellations_change_nothing`; `delete_all_needs_the_exact_word_and_clear_is_only_a_migration_notice` (padded, recased and EOF words are not the confirmation) |
| D03 | `keep_root_empties_a_topic_tree_and_keeps_its_root`; `keep_root_keeps_the_literature_root_identity_links_and_allocation` (row identity, address, kind, Workspace, BibTeX and text kept; the next successor is `<root>/1`; the global allocation unchanged); `keep_root_on_an_already_empty_tree_reports_zero_without_a_revision` |
| D04 | `keep_root_verification_names_the_root_workspace_count_and_breaking_links`; `keep_root_keeps_the_literature_root_identity_links_and_allocation` (an outside Link to the root stays valid, Links to deleted children break, the generated sections and the Link index stay exact) |
| D05 | `keep_root_is_allowed_while_a_session_holds_a_pointer_on_a_removed_child` (the live Session's Pointer recovers to the retained root) |
| D06 | `delete_all_refuses_an_open_session_without_changing_storage`; `delete_all_holds_session_registration_through_confirmation_and_commit`; `a_killed_delete_all_releases_both_guards_without_a_partial_delete` |
| D07 | `tests/tree_lock.rs::keep_root_refuses_a_locked_tree_even_when_empty_and_obeys_pins` (a Locked tree refuses emptying even with no Regular Card; a pin on the retained root alone permits it, a child pin refuses it) |
| D08 | `the_all_archive_delete_refuses_while_a_tree_is_locked_and_keeps_ended_lock_history`; `the_all_archive_delete_refuses_an_orphaned_lock_record` |
| D09 | `keep_root_rolls_every_effect_back_when_the_delete_fails`; `delete_all_rolls_back_cards_links_and_allocation_when_the_reset_fails` |
| D10 | `topic_delete_removes_topic_and_topic_ids_are_not_reused` (a subtree delete does not reuse Topic numbers); `keep_root_keeps_the_literature_root_identity_links_and_allocation` (keep-root leaves the allocation alone); `delete_all_empties_both_workspaces_resets_allocation_and_keeps_config_dumps_and_artifacts` (all-archive resets to `0`); `delete_all_advances_one_revision_for_a_change_and_none_for_an_empty_zero_archive` |
| D11 | `delete_all_empties_both_workspaces_resets_allocation_and_keeps_config_dumps_and_artifacts` (configuration, dumps, artifacts, receipts, ended Locks and other metadata survive); `delete_all_keeps_saved_spread_metadata_while_a_tree_delete_follows_positions` (all-archive keeps the stored Spread bytes); `keep_root_drops_a_deleted_card_from_saved_spread_positions` (keep-root keeps the retained root's position and drops the deleted Card's) |
| D12 | `delete_all_needs_the_exact_word_and_clear_is_only_a_migration_notice` (`zt clear` prints ``zt clear is now `zt del --all` `` and exits 1 without a service, prompt or deletion; `zt help` lists only the supported forms and no `zt clear`; the Session refuses `clear`); `tests/tree_lock.rs::the_lock_has_gui_routes_only_and_no_mcp_control_tool` (the MCP tool list and the GUI route set carry no Lock control and no remote delete-all authority) |

## 4. Delivery groups (R345–R351 / S18-AC01)

| Group | Requirements | Evidence |
|---|---|---|
| 1. Storage and gate | R341–R344, R379, R392 | `src/tree_lock.rs` unit tests: the four tables and their column sets, the SQL fingerprint equal to the Rust baseline for every field, seal framing, `zt_tree_root`, gate predicates, exact-set integrity, event deduplication, raw-connection abort and generated-refresh pass; `tests/tree_lock.rs` proves both Workspaces, both tree kinds and every writer path through the real GUI routes, the real MCP server, the real Session and the real shell |
| 2. GUI | R347 (slice #128 delivered it; this slice re-proves the backend it uses) | The real token-guarded `/api/locks`, `/api/lock`, `/api/unlock` and `/api/lock/note` routes in `tests/tree_lock.rs`: plan/`plan_changed`/commit, note, unlock, history, integrity, a stale draft, a plan recomputed from stored facts, and the 403 token check. Browser rendering and its controls belong to the main agent's GUI work; the page's own JavaScript is checked by the crate's unit tests |
| 3. Session, status and dump | R348 | `tests/tree_lock.rs`: `[locked]`/`[pinned]` marks in the line and TUI Session, a refusal before any editor or confirmation, `zt status`, `zt status --check-locks`, unreadable and faulted scopes, and `zt dp`'s `locks.json` evidence for active, ended, faulted and no-Lock archives |
| 4. Joined delete command | R349 | `tests/cli_service.rs` (the selector matrix, the four confirmation words, keep-root, all-archive, rollback, the edit-lock and open-Session refusals, the killed delete, Session registration) and `tests/tree_lock.rs` (D07/D08) |
| 5. MCP and agent integrations | R350 | `tests/tree_lock.rs` drives the stdio MCP server: structured `card_locked` refusals, both bulk read views, the lean read, `preview_items` aggregation, receipt replay, complete rollback, and the absence of any Lock-control tool; the installed skills and the MCP registration are re-installed and handshaked in §7 |
| 6. Workflows | R351 | `scripts/book-digest/tests/lock-preflight.tests.ps1`, `hub-lock.tests.ps1`, `organize-lock.tests.ps1`, `grow.tests.ps1`, `agent-lock-guidance.tests.ps1`, plus the three harnesses, all in the aggregate runner; the frozen `zt-econ-digest` 5.11.0, `zt-math-digest` 4.11.0 and `zt-idea-grow` 2.3.0 assemble and verify |

The design's §18.3 placements are satisfied, with one naming difference: the MCP, batch and preview evidence lives in `tests/tree_lock.rs` (which starts the real MCP server) rather than in `tests/workspaces_mcp.rs`; `tests/self_sufficient_models.rs` remains the placement suite for the non-Lock structural cases the design names, and the Lock placement cases are covered by the internal mapping tests plus the public T04 refusals. This keeps one fixture family per public seam without duplicating it. No test depends on the GUI JavaScript of slice 2 beyond the backend it calls.

## 5. Architecture and cost claims (R341–R344, R352, R353, R379 / S18-AC06)

- **Storage is O(M + P + E)** (§16, R341): `locks` has one row per Lock; `lock_members` one row per member holding `(lock_id, row_id, address, authored_fp)`; `lock_pins` one row per distinct `(source, target)` pair; `lock_events` one row per event. No table copies a Card body: `authored_fp` is a SHA-256 digest, and the initial event retains the member baseline and the seal, not the text. The claim is checked from `SCHEMA_TABLES_SQL` in `src/tree_lock.rs` and from every dump fixture that reads the four tables.
- **Creation and ordinary checks are indexed** (R342): `tree_lock::plan` reads the archive's Cards once, hashes the selected tree and verifies its outgoing Links once; the ordinary gate uses the four indexes (`locks_active_tree`, `lock_members_by_row`, `lock_pins_by_target`, `lock_events_by_lock`); a create checks one active root. No extra global permission scan exists; the existing structural Link regeneration is unchanged.
- **One integrity scan at named check points** (R343): `tree_lock::check` is the same comparison called by `zt status`, `zt status --check-locks`, the GUI panel and the startup guard. There is no poller, timer, worker thread or cached check result (the four tables are still exactly the evidence tables; `tests/tree_lock.rs::ordinary_status_reports_the_lock_scope_and_its_integrity` proves no check-result row exists).
- **Read annotations are bulk-loaded** (R352): `LockView::read` issues one pair of `IN`-list queries per read, chunked at 400 rows, and `writable_for` composes Workspace access with the Lock facts; `decorate_card` is the single helper that attaches `lock` and `pinned_by`.
- **Test-only Lock creation** (R379): the fixture helper calls `lock_tx` under the caller's edit lock and transaction inside `src/tree_lock.rs` `tests`; no hidden CLI Lock command exists (`the_shell_and_the_session_offer_no_lock_command`, `no_public_reseal_command_or_guard_bypass_is_shipped`).
- **No body copies, global per-read scan, new poller or generic framework** were added. The only new module is `src/tree_lock.rs`, assigned to the Service, CLI and storage block (§6).

## 6. Documentation contracts (R403–R414 / S18-AC04, S18-AC05)

| Contract | Where it now lives |
|---|---|
| Domain definitions (R404) | `CONTEXT.md`: **Lock**, **Locked tree**, **Pinned target** (shipped with their slices) and **Emptied tree** (this slice) |
| Storage, CLI and schema (R405) | `docs/wiki/service-and-cli.md`: the four tables, the schema version, the six triggers, the two SQL wrappers, boot steps, `zt status`, `zt status --check-locks`, `zt dp` and `locks.json`, `zt del` forms, keep-root, the clear notice |
| Card operations (R406) | `docs/wiki/card-operations.md`: the gate, the typed refusal, the delete scopes, complete mapping checks, preview and replay |
| Cards, Workspaces and Links (R407) | `docs/wiki/cards-and-links.md`: authored identity and fingerprint, Access composition, pins, the generated Reverse-link allowance |
| GUI (R408) | `docs/wiki/gui.md`: the Lock/note/unlock routes, the plan hash, the controls, history, integrity and draft preservation |
| Session (R409) | `docs/wiki/session.md`: `[locked]`/`[pinned]`, keep-root, prechecks, registration exclusion and refusal hints |
| MCP server (R410) | `docs/wiki/mcp-server.md`: read fields, `writable` meaning, error serialization and preview limits |
| Agent integrations (R411) | `docs/wiki/agent-skills.md`: the owner-only Lock policy and the updated skills |
| Digest workflows, scripts, `zt-digest`, idea-grow (R412) | `docs/wiki/digest-workflows.md`, `digest-scripts.md`, `zt-digest.md`, `idea-grow-workflow.md`: preparation and destination rules and the newly published frozen versions |
| Dump/delete design and README/help (R413) | `docs/dump-and-clear-design.md` (one delete command, keep-root, the `zt clear` notice and its next-release removal, `locks.json`); `README.md` and `README.zh-CN.md` now list the four `zt del` forms, the **Emptied tree** and the one-release `zt clear` notice; `zt help` advertises only the supported forms |
| Module assignment (R414) | `src/tree_lock.rs` is listed in the Service, CLI and storage block's `paths` and described there; its operation integration is cross-linked from Card operations and Cards/Links. No extra block was created |

The block's Testing rows record the Tree Lock tests, including this slice's new qualification cases (a D02 confirmation case and a D11 Spread case in `tests/cli_service.rs`; T04/T06, T22, the member title refusal and the D12 authority check in `tests/tree_lock.rs`). `wiki.mjs check` reports every block fresh after the stamp commit; the only repair this slice made to the block besides its own rows is a broken markdown table row (a stray bare CR split the `tests/tree_lock.rs` row in two since the S13 commit) and that file's CRLF line endings, now LF like every other text file in the repository.

## 7. Installed release qualification (R415 / S18-AC07 / S18-AC03)

- **Isolation.** Every Rust integration test and every digest suite starts its own service with a scratch `ZT_CONFIG_DIR`, a scratch archive root and `ZT_GUI_PORT=0`; no test locks, edits, empties or migrates the configured owner archive. All tampering fixtures run on temporary databases. The owner archive `D:\zt` was only read (status, integrity checks, one `VACUUM INTO` backup) and never locked or written.
- **Pre-upgrade backup.** The archive's consistent pre-upgrade copy is `D:\zt\zt.sqlite3.pre-s18-20261021-234844.bak`, written by `VACUUM INTO` with the service stopped: 161 Cards, `PRAGMA integrity_check` = `ok`, SHA-256 `8CF38E9B34503860B0A12BD740B58C4B27798DA1D6937A7C813A77AE9777B6A9`. The Tree Lock installation's own `backup_before_install` contract (`zt.sqlite3.pre-tree-lock-<s>.bak`) is proven by the unit test `the_first_install_backs_up_the_prior_archive_consistently`.
- **Installed binary.** `cargo build --release --locked` then `cargo install --path . --locked` from the release commit; the installed `zt.exe` matches the release artifact byte for byte, SHA-256 `0498043E4DBA325A21B9F693E35BC9D81499639A9909ABE155F9DCF83BC8C170` (the running image held by an open MCP client is replaced by renaming it aside first, the same procedure `install-zt-digest.ps1` documents). `zt-check` and `zt-digest` were rebuilt and installed from the same commit.
- **Service health and integrity** after the restart: `zt status` reports `state: up`, `daemon: version=0.26.0`, `cards: 161`, `locks: 0 trees, 0 cards, 0 pinned targets; integrity ok`, revision `4537`; `zt status --check-links` reports `ok: true`, 161 Cards, 680 Links, no faults; `zt status --check-locks` reports `integrity: ok`, `recorded: 0 new fault event(s)` and exits 0. The archive revision did not move, because the archive already carried the version-1 Lock schema and the release performs no migration.
- **Rollback.** The previous executables are retained beside the installed ones as `*.old-20261021-234837` / `*.held-20261021-234837`, and the pre-upgrade archive copy above restores the archive byte for byte. The release changed no archive schema, so an older binary reads the same archive; an older writer is nevertheless unsupported against a Locked archive (the design's rule), and no Lock exists on the live archive.
- **Clients and integrations.** `scripts/install-agent-integrations.ps1` re-installs the `zt` and `zettelkasten` skills for Claude Code, Codex and DeepSeek Harness, re-registers the `zt mcp` server and passes its MCP handshake; the workflow-framework protocol tests and the frozen `assemble verify` checks pass with the new binaries. Conversations that are already open keep the servers they started with, so a client restart is the operator's step; the installer says so itself.

## 8. Policy boundary (S18-AC09)

The guarantee is local and policy based. It covers writes through the supported `zt` paths and detects the specified drift at the named check points. It does not authenticate a deliberate local process as the human, does not continuously watch the database, does not freeze external source documents, and cannot prove historical integrity after an attacker rewrites both content and baseline evidence. A deliberate token or SQL bypass is outside the promise even though the triggers still try to stop it; a SQLite client that reaches a trigger message must abandon its enclosing transaction. Successful design and issue publication alone never established implementation or installed qualification; the evidence for this release is the suites in §2, the scenario table in §3 and the installed-binary facts in §7.

## 9. Follow-ups (S18-AC08)

- The next release removes the `zt clear` notice handler, leaving an unknown command; `docs/dump-and-clear-design.md` records this as the documented follow-up, and `zt help` already hides `zt clear`.
- No restore command, partial or time-expiring Lock, force bypass, automatic digest Lock, historical bulk edit or production owner Lock operation is part of completion. S04 (#127) remains its own later issue; its note editing and retained-history inspection are not part of this release qualification.

## 10. Environment limits and findings

- The PowerShell suites need PowerShell 7.5+ and, for the dump-format cases, a host archiver; they ran on Windows. One PTY clipboard test is marked ignored by the suite's own design.
- The digest suites resolve `zt-digest`/`zt-check` from `PATH`; a folder passed as `-Zt` that also contains stale tool binaries shadows them. The first aggregate run used `D:\zt-target\debug\zt.exe`, whose folder held a `zt-digest.exe` built before the frozen versions 5.11.0/4.11.0, so `ledger-preparation` and `pilot` saw a contract frozen at 5.9.9 and failed. The tool binaries were rebuilt from the release commit and the final aggregate run passes the installed release binaries instead.
- The same first run exposed a test-isolation defect in `scripts/book-digest/tests/bridge-attribution-retry.tests.ps1`: a recycled process id could find the archive an earlier run left in its scratch folder and then create its own Literature Card twice. The suite now clears its scratch root first, like the other fixtures do.
- `cargo install` over a running service cannot replace an executable held by an open MCP client; the install procedure renames the held file aside first. The running service itself is stopped with `zt down` before the swap.
- No real book digest, digest stage or owner Lock was run: the archive still has zero Locks, as it had before the release.
