# Dump and Clear Design

Status: locked and complete for `zt dp`; the `zt clear` decisions are superseded by the one `zt del`
command of the Tree Lock feature (last section).

## Source Authority

- `CONTEXT.md` defines the project language. This document uses `Location` for the persisted card address rather than `zt id string`.
- `docs/zt-note-system-design.md` and `docs/zt-note-system-prd.md` are the current locked product contract.
- `src/main.rs` owns shell command dispatch, configuration, service state checks, SQLite initialization, card storage helpers, and Topic ID allocation; `cmd_dump` loads every Card and hands it to `src/dump.rs`.
- `src/dump.rs` owns the dump payload, the archive manifest, compression detection, and archive cleanup.
- `src/session.rs` owns interactive Session command dispatch. Shell-only commands should not be added to Session help or Session command handling.

## Current Code Facts

- Card storage is one SQLite table named `cards`. A Topic or Regular Card has a `location`; a Literature Card has a `citation_key` and `bibtex` instead. Every Card has its `text`, its `workspace` (`human` or `agent`), and its `card_type` (`idea`, `event`, or `model`).
- Topic root allocation is tracked through the `metadata` key `next_topic_id`; if missing, the code falls back to max existing Topic ID plus one.
- The SQLite database lives at `<archive_root>/zt.sqlite3`.
- Existing shell commands that read card data require the service to be up through `require_service_up()`.
- Existing command availability while the service is down is intentionally limited to lifecycle, status, version, help, and config commands.
- The project currently has no Rust compression/archive dependency in `Cargo.toml`.

## Locked Decisions

- Add a shell command named `zt dp` for full storage dump.
- `zt dp` is a CLI-only command. It must not be callable from inside an interactive Session command bar.
- A dump run captures the current Unix timestamp at seconds precision once and uses that timestamp for all names produced by the run.
- The logical archive folder name is `zt-archive-<unix-sec-timestamp>`.
- A dump has exactly one payload schema: a compressed archive whose top-level entry is the folder `zt-archive-<unix-sec-timestamp>`.
- Inside that top-level folder there is `mapping.json`, `archive.json`, one Markdown file per Card row, and one `<citation-key>.bib` file per Literature Card.
- Each Markdown file stores exactly the `cards.text` value from SQLite for one Card. A Literature Card's file first carries YAML front matter (`zt_kind: literature`, `citation_key`, and its `bibtex` as a block scalar that keeps the exact trailing newlines), then the exact `cards.text`.
- `<citation-key>.bib` holds the exact BibTeX bytes of that Literature Card.
- `archive.json` is the manifest of everything that is not Card text: `next_topic_id`, `workspaces` (every address in `mapping.json` mapped to `human` or `agent`), and `types` (every Event and Model Card mapped to its Card type; Idea Cards are left out). Its keys are written in that order.
- A dump therefore holds enough to replicate the archive: every Card's text, address, Workspace, and Card type, every Literature Card's BibTeX, and the next Topic number.
- Each Markdown filename is a 10-character hash plus the `.md` suffix.
- The hash input includes the Card address (its `Location`, or a Literature Card's Citation key) and the exact `cards.text` value, so two Cards with identical text still produce different Markdown filenames because they have different addresses.
- The JSON maps each Card address to the Markdown filename for that Card, including the `.md` suffix.
- The JSON mapping file is named `mapping.json`.
- `mapping.json` is a single JSON object whose keys are Card addresses and whose values are Markdown filenames.
- `mapping.json` contains only the Location-to-Markdown-filename mapping object and no extra metadata.
- Markdown filenames are derived from SHA-256 over the address, one NUL byte, and the exact `cards.text` value (for a Literature Card: the Citation key, one NUL byte, the exact BibTeX, one NUL byte, and the exact `cards.text`), hex-encoded and truncated to 10 characters before adding `.md`.
- If two Cards produce the same 10-character Markdown filename hash, `zt dp` resolves the collision by appending two random 10-character strings after the text in the hash input and recomputing.
- Collision recomputation repeats until the 10-character Markdown filename hash is unique within the dump.
- The random 10-character collision strings use lowercase hexadecimal characters from OS randomness.
- Collision retry salts are not recorded in `mapping.json` or anywhere else in the archive payload.
- Dump output ordering is deterministic.
- Deterministic dump output ordering does not promise byte-identical archive contents if a rare 10-character hash collision forces random collision retry salts.
- `zt dp` orders Cards by address.
- `zt dp` writes `mapping.json` keys in address order.
- `zt dp` creates archive entries in stable order: `mapping.json`, `archive.json`, the Markdown files in address order, then the `.bib` files in Citation key order.
- `zt dp` may dump empty storage.
- When storage is empty, `zt dp` creates an archive containing `mapping.json` as `{}`, `archive.json` with empty `workspaces` and `types`, and no Markdown files.
- On success, `zt dp` prints the archive path and dumped Card count.
- The success output shape is `dumped <count> cards to <archive-path>`.
- The compressed archive is stored under `<archive_root>/dump/`.
- The compressed archive filename uses the same timestamp base plus the selected compression extension: `zt-archive-<unix-sec-timestamp>.zip`, `zt-archive-<unix-sec-timestamp>.tar.gz`, `zt-archive-<unix-sec-timestamp>.tar.zst`, or `zt-archive-<unix-sec-timestamp>.7z`.
- If the compressed archive output path already exists, `zt dp` fails without overwriting it.
- The intermediate folder is created under `<archive_root>/dump/.tmp/zt-archive-<unix-sec-timestamp>/`.
- The intermediate uncompressed folder is removed after the compressed archive is created.
- The intermediate folder is also removed after dump failure.
- If the selected compression command fails after writing starts, `zt dp` removes the intermediate folder and any partial compressed archive file, then exits nonzero with the compression error.
- `zt dp` requires the service to be up.
- Dump compression must not be hard-coded to one fixed format. The system detects usable compression options on the host and lets the client choose a detected option.
- `zt dp` does not accept a compression flag.
- `zt dp --compress <option>` is refused rather than accepted as a compression-selection interface.
- Compression selection is an interactive shell prompt inside the `zt dp` command.
- The `zt dp` prompt lists only detected usable compression options.
- The user chooses one of the listed compression options by responding to the prompt.
- The `zt dp` prompt lists compression options as numbered choices.
- The `zt dp` prompt accepts only the number of a listed compression option.
- If the user enters anything other than a listed number, `zt dp` fails without dumping.
- If no recognized compression option is usable on the host, `zt dp` fails before creating the intermediate folder.
- The no-compression-available failure says no supported compression option was detected and lists the supported option names.
- The recognized compression option names are `zip`, `tar.gz`, `tar.zst`, and `7z`.
- Each recognized compression option is available only when its backing system command is detected.
- A compression option is usable only if detection proves that the backing command can create that archive format, not merely that the executable exists: the command compresses a small probe folder, and the result must start with the format's signature (`PK` for zip, `1f 8b` for gzip, `28 b5 2f fd` for zstd, `7z bc af 27 1c` for 7z).
- A host may hold several executables of one name. Detection tries every executable of that name on PATH, in PATH order, and the first one that proves the format is the one the dump runs. Git for Windows, for example, puts a GNU tar ahead of Windows' bsdtar on PATH; GNU tar cannot write zip (it writes a plain tar named `.zip`), so the dump falls through to bsdtar instead of failing or writing the wrong format.
- Before the archive is published, `zt dp` checks it carries the selected format's signature again, and fails with cleanup if it does not.
- On Windows, `zip`, `tar.gz`, and `tar.zst` are detected through usable `tar.exe` support for those formats. `tar.exe` runs from inside the archive's folder and names the archive by its file name alone, because a GNU tar reads a `C:\...` archive path as a remote host and stalls before failing.
- On Windows, `7z` is detected through `7z.exe` or `7zz.exe`.
- On Unix-like systems, `zip` is detected through `zip`.
- On Unix-like systems, `tar.gz` is detected through `tar` plus `gzip`.
- On Unix-like systems, `tar.zst` is detected through `tar` plus `zstd`.
- On Unix-like systems, `7z` is detected through `7z` or `7zz`.
- `zt dp` may run while interactive Sessions are open.
- `zt dp` refuses to run while the edit lock exists.

### Lock evidence in the dump (Tree Lock §14.3, #136)

- The dump holds a third named payload entry, `locks.json`, written beside `mapping.json` and
  `archive.json` in every archive format: staging and the explicit compressed-entry list both name
  it, immediately after `archive.json`, before the Card files, `.bib` files and artifacts.
- `locks.json` is always written, including when the archive holds no Lock: it then carries empty
  `locks` and `events` arrays, so a reader never has to guess whether an archive was inspected for
  Locks.
- Its `schema_version` is the export format version `1`. It also carries `archive` (the stored
  archive identity) and `revision` (the captured archive revision).
- It lists every Lock, active or ended, in Lock-id order with its seal, root address, root row,
  tree id, Workspace, note, member count and `unlocked_at`.
- It lists the live member snapshots of the active Locks (portable address, audit row identity and
  authored fingerprint) in address order, and their live pins (source and target addresses with
  both row identities) ordered by target address, target row, source address and source row. An
  ended Lock exports empty arrays: unlocking deletes its live rows, and its baseline stays in its
  initial `lock` event.
- It lists every event of every Lock in event-id order with its stored JSON `detail`, so an ended
  Lock's initial snapshot stays readable after its old tree is renamed or deleted.
- It lists the integrity facts of the same captured state: whether the promise holds, the current
  faults with their expected and observed facts, and the retained latched evidence.
- Addresses and Lock ids carry the portable meaning. Row numbers are audit evidence inside that
  captured database; a future restore must resolve them explicitly rather than assuming they are
  new database row ids.
- The Lock evidence is read while the dump holds the edit lock, in the same SQLite view as the
  Card texts, `archive.json` and `next_topic_id`, and is passed to `src/dump.rs` as an explicit
  value. The dump module stays a serializer/compressor: it opens no second connection to discover
  Locks later.
- An integrity fault is exported as evidence and does not prevent a diagnostic dump. Lock data
  that cannot be read or serialized fails the whole dump, which leaves no staging folder and no
  archive behind.
- The dump makes Lock history inspectable. This feature adds no restore command and promises no
  automatic future import of Locks.

## Superseded delete decisions (`zt clear` → `zt del --all`)

The Tree Lock feature folded `zt clear` into one delete command ([Tree Lock Implementation
Design](tree-lock-implementation-design.md) §12, slices S11/S12, #134/#135). The decisions above
about `zt dp` still hold; the `zt clear` block that used to sit here is replaced by this contract:

- Shell `zt del` has exactly four forms: `--at <card>`, `--at <root>`, `--at <root> --keep-root`, and
  `--all`. Exactly one selector is required; a repeated selector or flag, an unknown argument, both
  selectors, and `--keep-root` beside `--all` are refused with
  `usage: zt del --at <target> [--keep-root] | zt del --all` before the service, a plan or a prompt.
- The confirmation words are `delete` (Regular Card subtree), the root's own address (whole tree and
  keep root), and `delete all` (whole archive). Exactly one stdin line is read and only its line
  terminator is removed; any other word or EOF cancels with no SQLite change.
- `zt del --all` is the old clear: it deletes every row of `cards` and of the derived `links` index,
  resets `metadata.next_topic_id` to `0`, and prints
  `deleted: <count> cards; next topic id reset to 0` in one transaction. It refuses while a Session
  is open — through the existing `zt.sessions.lock`, held from that check through the commit — and
  it refuses while any Lock is active, from the Lock records alone, before the prompt.
- `zt del --all` keeps configuration, `<archive_root>/dump/`, `artifacts/`, every other metadata row
  (including the saved Spreads), the `applied_ops` receipts, and the ended Locks with their history.
  Tree and keep-root deletes still let Spreads follow the deleted positions.
- Only `zt del --all` reuses Topic numbers. An archive that already holds no Card but a nonzero
  allocation still advances the archive revision once when it is reset; an archive with neither
  succeeds and changes nothing.
- `zt clear` is no longer a destructive command. For one release it prints
  ``zt clear is now `zt del --all` `` and exits 1 without requiring the service, prompting or
  deleting anything, and it is not listed by `zt help`. The next release removes the notice handler,
  leaving an unknown command; this is a documented follow-up, not a working alias.
- Shell `zt help` lists the delete forms while the service is up and no longer lists `zt clear`.
  Session `help` lists `del [keep-root]` and no delete-all form; the Session refuses `clear` as an
  unknown command.

## Open Decisions

None currently identified.
