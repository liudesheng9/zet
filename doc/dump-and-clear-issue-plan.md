# Dump and Clear Issue Plan

Parent PRD: [#22](https://github.com/liudesheng9/zet/issues/22)

Source design: [dump-and-clear-design.md](dump-and-clear-design.md)

This four-issue breakdown is self-approved under the user's instruction to approve the issues. Each issue is an AFK-ready tracer-bullet slice published with the `ready-for-agent` label.

## Published Issues

- [#22 PRD: Full Card dump and atomic storage clear](https://github.com/liudesheng9/zet/issues/22)
- [#23 Export complete Card storage through zt dp using a detected ZIP path](https://github.com/liudesheng9/zet/issues/23)
- [#24 Select every supported host compression format interactively](https://github.com/liudesheng9/zet/issues/24)
- [#25 Harden zt dp concurrency and archive cleanup](https://github.com/liudesheng9/zet/issues/25)
- [#26 Clear Card storage and reset Topic allocation atomically](https://github.com/liudesheng9/zet/issues/26)

## Vertical Slices

1. Export complete Card storage through `zt dp` using a detected ZIP path
   - Type: AFK
   - Blocked by: None
   - Covers user stories 1-14, 21, 23, 26, 39, and 40.
   - Delivers the first complete shell-to-archive path: command boundary, service gate, timestamped archive shape, exact Card Markdown payloads, ordered Location mapping, SHA-256 filenames with collision handling, empty storage, output text, and archive inspection tests.

2. Select every supported host compression format interactively
   - Type: AFK
   - Blocked by: #23
   - Covers user stories 15-20 and completes format behavior for stories 11-14 and 26.
   - Expands the working dump path to capability-probed `zip`, `tar.gz`, `tar.zst`, and `7z` choices on Windows and Unix-like hosts, with a numbered prompt, strict input, explicit refusal of `--compress`, correct extensions, and no-option behavior.

3. Harden dump concurrency and archive cleanup
   - Type: AFK
   - Blocked by: #23 and #24
   - Covers user stories 9, 14, and 21-25.
   - Makes every compression path safe under existing destinations, open Sessions, edit locks, failed compressors, and partial output, while preserving stable archive-entry order and proving cleanup through CLI integration tests.

4. Clear Card storage and reset Topic allocation atomically
   - Type: AFK
   - Blocked by: None
   - Covers user stories 27-40 as they apply to clear.
   - Delivers shell-only `zt clear` with exact confirmation, Session/edit-lock refusal, one-transaction Card deletion and metadata reset, first-use Topic allocation, success/cancel output, and proof that configuration and prior dumps remain unchanged.

## Design Coverage

| Locked design area | Covered by | Required proof |
| --- | --- | --- |
| Shell-only `zt dp`, service-up availability, shell and Session help boundary | Slice 1 | CLI and Session integration tests |
| Single timestamp, archive/folder naming, top-level folder, dump destination | Slice 1 | Archive-path and archive-entry assertions |
| One Markdown file per Card with exact SQLite text | Slice 1 | Byte-for-byte archive payload assertions |
| `mapping.json` name, mapping-only object, ordered Location keys and `.md` values | Slice 1 | Parsed JSON and ordering assertions |
| SHA-256 `Location + NUL + text`, 10-character lowercase hex filenames | Slice 1 | Known-vector tests and archive assertions |
| Identical text at different Locations and collision retry with two OS-random lowercase-hex salts | Slice 1 | Multi-Card integration test and forced-collision lower-level test |
| Deterministic Card, mapping, and archive-entry ordering, with collision caveat | Slices 1 and 3 | Repeated dump and entry-order assertions |
| Empty storage archive and exact dump success output | Slice 1 | Empty SQLite CLI integration test |
| Numbered interactive selection with no compression flag | Slice 2 | Stdin and argument-refusal integration tests |
| Real capability probes and detected-options-only prompt | Slice 2 | Controlled executable/probe tests |
| Windows `tar.exe` and `7z.exe`/`7zz.exe` matrix | Slice 2 | Platform-routed command tests |
| Unix-like `zip`, `tar`+`gzip`, `tar`+`zstd`, and `7z`/`7zz` matrix | Slice 2 | Platform-routed command tests |
| Invalid selection and no-supported-option failure before temporary output | Slice 2 | Filesystem-negative integration tests |
| Correct `.zip`, `.tar.gz`, `.tar.zst`, and `.7z` archive names | Slice 2 | Final-path assertions for every option |
| Existing destination refusal, success/failure temporary cleanup, partial archive cleanup | Slice 3 | Preexisting-file and failing-compressor integration tests |
| Dump allowed with open Sessions and refused under edit lock | Slice 3 | Session-count and edit-lock integration tests |
| Shell-only `zt clear`, service-up requirement, exact confirmation and output | Slice 4 | CLI stdin and help tests |
| Clear refusal with any Session or edit lock | Slice 4 | Live Session/edit-lock integration tests |
| One-transaction Card deletion and `next_topic_id = 0` reset | Slice 4 | SQLite and failure-atomicity assertions |
| Zero stats, next Topic `0/0`, first regular Card `0/1` | Slice 4 | End-to-end clear/create workflow |
| Preserve config and `<archive_root>/dump/`; no required pre-clear dump | Slice 4 | Before/after byte and filesystem assertions |

## Acceptance Gate

- All four GitHub issues carry `ready-for-agent`.
- Every issue body references the parent PRD and names its blockers with live issue numbers.
- The four issue bodies collectively retain every requirement in the coverage table.
- `cargo test` is the implementation-wide automated gate after all four slices land.
