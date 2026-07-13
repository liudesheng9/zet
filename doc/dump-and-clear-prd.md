# Dump and Clear PRD

## Problem Statement

`zt` stores the full Card graph in SQLite, but users do not have one stable, portable archive format for exporting every stored Card. A reliable dump must preserve exact Card text, retain the relationship between each Card Location and its exported Markdown file, work with compression tools already usable on the host, and leave only the finished archive behind.

Users also need a deliberate way to reset Card storage to first-use state without deleting configuration or previous dumps. Deleting Cards one at a time does not reset Topic allocation, and a destructive reset must not race with an active Session or edit.

## Solution

Add two shell-only commands.

`zt dp` exports the current Card storage into a timestamped archive under `<archive_root>/dump/`. It builds one top-level folder containing `mapping.json` and one exact-text Markdown file per Card, detects which supported compression formats can actually be created on the host, prompts the user to select one detected option, creates the archive, and removes all intermediate output.

`zt clear` asks for explicit confirmation, then atomically deletes every Card and resets Topic allocation to zero. It leaves configuration and all dump archives untouched. The next Topic after a clear starts at Location `0/0` and its first regular Card remains `0/1`.

## User Stories

1. As a `zt` user, I want to run `zt dp` from the shell, so that I can export all stored Cards.
2. As a `zt` user, I want `zt dp` unavailable inside a Session, so that destructive and operational commands retain a clear shell boundary.
3. As a `zt` user, I want every stored Card represented by one Markdown file, so that no Card disappears from the dump.
4. As a `zt` user, I want each Markdown file to contain the exact SQLite Card text, so that the archive preserves the stored representation without transformation.
5. As a `zt` user, I want `mapping.json` to map every Card Location to its Markdown filename, so that exported Cards remain addressable.
6. As a `zt` user, I want the mapping file to contain only the Location-to-filename object, so that the dump has one simple schema.
7. As a `zt` user, I want identical Card text at different Locations to produce distinct filenames, so that both Cards remain present.
8. As a `zt` user, I want rare truncated-hash collisions resolved safely, so that one Card can never overwrite another in a dump.
9. As a `zt` user, I want Card loading, mapping keys, and archive entries ordered consistently, so that dumps are easy to inspect and test.
10. As a `zt` user, I want empty storage to produce a valid archive with an empty mapping, so that dump behavior does not depend on Card count.
11. As a `zt` user, I want one seconds-level Unix timestamp used throughout a dump, so that its folder and archive names agree.
12. As a `zt` user, I want the archive to contain one timestamped top-level folder, so that extracting it does not scatter files.
13. As a `zt` user, I want completed archives stored under `<archive_root>/dump/`, so that dumps are separated from live storage.
14. As a `zt` user, I want no uncompressed intermediate folder left after a dump, so that temporary data does not accumulate.
15. As a `zt` user, I want `zt dp` to detect usable host compression formats, so that it does not offer commands that cannot create an archive.
16. As a `zt` user, I want to choose a detected compression format from a numbered prompt, so that format selection is explicit for each dump.
17. As a `zt` user, I want only proven usable options listed, so that a listed choice is expected to work.
18. As a `zt` user, I want `zt dp --compress <option>` refused, so that the command has one compression-selection interface.
19. As a `zt` user, I want invalid prompt input rejected before dumping, so that an accidental choice does not create an archive.
20. As a `zt` user, I want a clear error when no supported compressor is usable, so that I know which formats the command recognizes.
21. As a `zt` user, I want existing archive paths protected from overwrite, so that a same-second filename collision cannot destroy a dump.
22. As a `zt` user, I want partial output removed when compression fails, so that failed archives are not mistaken for valid dumps.
23. As a `zt` user, I want `zt dp` to require the service to be up, so that command availability follows the existing data-command contract.
24. As a `zt` user, I want `zt dp` to work while Sessions are open, so that read-only exporting does not interrupt navigation.
25. As a `zt` user, I want `zt dp` refused while the edit lock exists, so that it cannot capture a Card during an active edit.
26. As a `zt` user, I want successful dump output to report the Card count and archive path, so that I can verify what was created.
27. As a `zt` user, I want to run `zt clear` from the shell, so that I can intentionally reset Card storage.
28. As a `zt` user, I want `zt clear` unavailable inside a Session, so that storage reset cannot be triggered from the command bar.
29. As a `zt` user, I want to type exactly `clear` before storage is erased, so that accidental invocation is harmless.
30. As a `zt` user, I want any other confirmation input to cancel without changing SQLite, so that confirmation is fail-closed.
31. As a `zt` user, I want `zt clear` refused while a Session is open or an edit is active, so that reset cannot race with live work.
32. As a `zt` user, I want Card deletion and Topic-allocation reset committed in one transaction, so that partial reset state cannot persist.
33. As a `zt` user, I want `zt stats` to report zero Cards after clear, so that reset is immediately observable.
34. As a `zt` user, I want the next Topic after clear to be `0/0`, so that storage behaves like first use again.
35. As a `zt` user, I want the first regular Card after clear to remain `0/1`, so that existing Location rules remain intact.
36. As a `zt` user, I want `zt clear` to preserve configuration, so that the service remains configured after reset.
37. As a `zt` user, I want `zt clear` to preserve `<archive_root>/dump/`, so that backups survive live-storage reset.
38. As a `zt` user, I want clear to remain independent from dump, so that reset never forces or requires an archive.
39. As a `zt` user, I want shell help to list both commands only while the service is up, so that help reflects command availability.
40. As a Session user, I want Session help to omit both commands, so that it shows only valid Session subcommands.

## Implementation Decisions

- The command names are shell `zt dp` and shell `zt clear`.
- Neither command is added to Session dispatch or Session help.
- Both commands require the service to be up and use nonzero exit codes on failure.
- A dump captures the current Unix timestamp in seconds once and reuses it for its logical folder, temporary folder, and final archive base name.
- The archive's top-level entry is `zt-archive-<timestamp>`.
- The top-level folder contains exactly `mapping.json` plus one Markdown file for each Card row.
- Markdown file contents are the exact stored `cards.text` values.
- `mapping.json` is one JSON object mapping Location strings to filenames including `.md`, with no metadata fields.
- Cards are loaded in Location order, JSON keys are emitted in Location order, and archive entries are created in a stable order.
- A Markdown filename is the first 10 lowercase hexadecimal characters of SHA-256 over the UTF-8 Location, one NUL byte, and the exact UTF-8 Card text, followed by `.md`.
- If a 10-character filename collides within one dump, two independent random 10-character lowercase hexadecimal salts from OS randomness are appended after the text in the hash input and the hash is recomputed until unique.
- Collision salts are not stored in the archive. Stable ordering does not promise byte-identical archives in the rare collision-retry case.
- Empty storage produces `mapping.json` containing `{}` and no Markdown files.
- Supported compression option names and extensions are `zip`, `tar.gz`, `tar.zst`, and `7z`.
- Compression selection is a numbered stdin prompt inside `zt dp`; no compression-selection flag is accepted.
- The prompt lists only options whose backing commands pass a real archive-creation capability probe.
- On Windows, ZIP, `tar.gz`, and `tar.zst` support is probed through `tar.exe`; `7z` is probed through `7z.exe` or `7zz.exe`.
- On Unix-like systems, ZIP uses `zip`; `tar.gz` uses `tar` and `gzip`; `tar.zst` uses `tar` and `zstd`; `7z` uses `7z` or `7zz`.
- Invalid selection fails without creating an intermediate folder or archive.
- No detected option fails before intermediate creation and reports all four supported option names.
- Final archives are written under `<archive_root>/dump/` with the selected extension.
- Intermediate files live under `<archive_root>/dump/.tmp/zt-archive-<timestamp>/` and are removed after success or failure.
- Existing destination archives are never overwritten.
- Compression failure removes both the intermediate folder and any partial destination archive before returning the compression error.
- A successful dump prints `dumped <count> cards to <archive-path>`.
- Dump is allowed while Sessions are open but refused while the global edit lock exists.
- Clear is refused while any Session is open and while the global edit lock exists.
- Clear reads one line from stdin without requiring an interactive terminal and proceeds only when it equals `clear` exactly.
- Any other clear confirmation prints `clear canceled` and leaves SQLite unchanged.
- Clear runs one SQLite transaction that deletes all `cards` rows and upserts `metadata.next_topic_id` to `0`.
- A successful clear prints `cleared <count> cards; next topic id reset to 0`.
- Clear does not read, remove, or modify configuration or the dump directory, and it never invokes dump.
- After clear, the existing Topic and Card creation rules allocate `0/0` and then `0/1`.

## Testing Decisions

- Use the existing CLI integration harness as the highest seam. Run the real `zt` binary against isolated configuration, archive roots, SQLite files, stdin, and temporary `PATH` contents.
- Inspect completed archives to verify the timestamped top-level folder, exact Markdown bytes, mapping shape, mapping values, empty-storage shape, and stable entry order.
- Use controlled fake compressor executables to prove capability probing, numbered selection, command choice, invalid selection, no-option behavior, partial-output cleanup, and platform-specific command routing without requiring every compressor on the test host.
- Keep focused lower-level tests for SHA-256 input construction and a forced collision path with injected randomness because a real 10-character SHA-256 collision is not practical to generate in an integration test.
- Test dump against zero Cards, one Card, multiple Cards, identical text at different Locations, Unicode text, embedded newlines, and forced truncated-hash collision.
- Test each supported format's extension and backing command selection, including `7z`/`7zz` fallback.
- Test service-down refusal, edit-lock refusal, dump with an open Session, existing destination refusal, compression failure cleanup, and success output.
- Test clear cancellation and success through stdin, and assert SQLite Cards, metadata, `zt stats`, the next Topic and regular Card Locations, config bytes, and existing dump bytes.
- Test clear refusal with an open Session and with an edit lock, with no storage changes.
- Test shell help while the service is up and down separately from Session help.
- Prefer observable command, archive, filesystem, and SQLite outcomes over assertions about private function structure.

## Out of Scope

- Importing or restoring a dump.
- Automatically dumping before clear.
- Accepting a noninteractive compression flag or configuration item.
- Running dump or clear from a Session.
- Encrypting, signing, uploading, or rotating archives.
- Adding archive metadata beyond the Location mapping.
- Guaranteeing byte-identical archives after a forced hash-collision retry.
- Changing Card text, Location grammar, Topic creation rules, configuration schema, or unrelated Session behavior.
- Adding general SQLite migrations as part of this feature.

## Further Notes

This PRD is based on the locked decisions in `doc/dump-and-clear-design.md`. The four approved AFK slices are: a complete ZIP-backed dump path, the full supported compression matrix and selector, dump lifecycle and concurrency hardening, and atomic storage clear.
