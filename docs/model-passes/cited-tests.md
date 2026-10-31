# zt-digest Cited tests: the stale `zt` binary (2026-10-31)

## Symptom

In `tools/zt-digest`, `cargo test --test cited` failed 2 of 6 tests and `--test cited_cards` failed 6 of 6 with `zt-digest cited: zt: card `` does not exist`, from the zt seam. They passed at zt-digest 0.37.8 (4bcda3c).

## Cause

The cause is the `zt` binary the tests started. The zt-digest code is not involved. Each integration test file had its own `zt_program()` with a fixed order: `ZT_TEST_ZT`, then `E:\zet\target\debug\zt.exe`, then `target\release\zt.exe`, and only after those the `zt` on PATH. `target\debug\zt.exe` is a stale build from 2026-10-26, `zt 0.42.0`. It predates the Cited Card type and Cited list support. The installed zt (`~/.cargo/bin/zt`) is `zt 0.49.0`. So the seams ran a zt that cannot write Cited Cards. The passing run at 0.37.8 most likely set `ZT_TEST_ZT` or had no usable repository build. I could not establish which.

Evidence:
- With the default choice, `cited` failed 2/6. With `ZT_TEST_ZT=<path of zt 0.49.0>` and the same working tree, `cited` passed 6/6 and `cited_cards` passed 6/6.
- That working tree includes the uncommitted zt-digest 0.37.9/0.37.10 changes in `src/ledger/adopt.rs` and `src/model/compiler.rs`. Those changes are not the cause, and this job did not touch them.

## Fix (test code only, test-first)

- New shared helper `tests/support/zt_program.rs`. It returns `ZT_TEST_ZT` when that is set. Otherwise it runs `zt version` on each candidate (`target/debug`, `target/release`, PATH `zt`) and returns the one with the highest version. Versions compare as numbers, and on a tie the earlier candidate wins, which keeps the old preference for repository builds of the same version. A stale repository build can no longer win over a newer installed zt.
- New test file `tests/zt_program.rs`, written first and red: 3 tests (version parsing, newest wins over a stale build with 0.10 > 0.9, tie and empty). It is now green.
- The 12 integration test files that each had their own copy of `zt_program()` now include the shared helper (`#[path = "support/zt_program.rs"] mod zt_program;`): cited, cited_cards, model_explain_batch, model_one_conversation, term_backfill, term_cards_apply, term_cards_check, term_cards_plan, term_cards_precheck, term_link_apply, term_link_plan, term_pilot. Net change in those 12 files: -231 / +36 lines, plus the two new files.

No source file of zt-digest changed, no version bump, no redeploy. The live zt does not need a change.

## Result

`CARGO_TARGET_DIR=D:\zt-digest-target cargo test --no-fail-fast` with no `ZT_TEST_ZT` passes in full: 611 unit tests, 15 integration binaries (cited 6, cited_cards 6, coverage 6, model_explain_batch 2, model_one_conversation 2, term_backfill 11, term_cards_apply 9, term_cards_check 2, term_cards_plan 1, term_cards_precheck 1, term_link_apply 7 + 1 ignored (an `#[ignore]` that was there before), term_link_plan 1, term_pilot 1, zt_program 3).

## Left for the main session

- Nothing is committed. Changed files: `tools/zt-digest/tests/*.rs` (12 edited, `zt_program.rs` new) and `tools/zt-digest/tests/support/zt_program.rs` (new).
- The wiki block doc for zt-digest (`docs/wiki/zt-digest.md`, already modified by another worker) could get one line on how the integration tests choose `zt`. I did not edit it, because shared docs are not mine to edit.
- `E:\zet\target\debug\zt.exe` (0.42.0) is still on disk. With the fix it does no harm. Deleting or rebuilding it is optional.
