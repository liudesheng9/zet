# Literature Card Design

Status: design locked for implementation.

## Source Authority

- `CONTEXT.md` defines the project language. This document uses `Regular Card`, not "normal card", and reserves `Location` for the existing Topic-tree address system.
- `doc/zt-note-system-design.md` and `doc/zt-note-system-prd.md` are the current Card, Topic, Link, Reverse link, storage, and Session contract.
- `doc/dump-and-clear-design.md` is the current dump and destructive-clear contract.
- `doc/session-bare-subcommands-design.md` is the current Session command syntax contract.
- `src/main.rs`, `src/session.rs`, and `src/dump.rs` are the current implementation authority where older documentation and live behavior differ.

## Conflict Precedence

- This document supersedes older contracts only where Literature Cards require a change.
- It supersedes the old three-column-only `cards` schema, Location-only Link and `go` target rules, the prohibition on all Topic outbound Links, the old three-way `stats` count, and Location-only dump mapping.
- It adds a Literature-specific exception to the rule that every dumped Markdown file is exactly `cards.text`; Regular and Topic Card Markdown files remain unchanged.
- Existing Card text, Location topology, Session bare-command syntax, move/delete compaction, service lifecycle, edit lock, dump compression, and clear-safety rules remain authoritative unless explicitly changed here.

## Current Code Facts

- SQLite has one `cards` table with `location TEXT PRIMARY KEY`, `is_topic`, and `text`.
- Card loading, saving, navigation, Link validation, Reverse link generation, delete, move, listing, and dump currently identify every Card by `location`.
- A Link is written as `[[location]]`. Link parsing currently accepts only the existing numeric Location grammar.
- Topic descriptions currently reject all outbound Link macros.
- Reverse links are recomputed from Link macros in Card bodies and written into the third text section of every Card.
- Both TUI and piped Sessions use bare Session subcommands. Bare `go` returns the Pointer to `ROOT`, while `go <target>` resolves an existing Location or Citation key.
- Shell dump is `zt dp`. It writes one Markdown file containing exactly `cards.text` per Card, hashes `Location + NUL + text`, and maps each Location to its 10-character-hash filename in `mapping.json`.
- The existing destructive command is `zt clear`, not `zt clean`. It deletes every row from `cards` and resets Topic allocation.
- The project is still in its development-period no-migration phase unless this design explicitly changes that rule.

## Locked Requirements

- Add Literature Cards as a Card kind alongside Topic Cards and Regular Cards.
- A Literature Card represents one bibliographic work.
- A Literature Card is outside the existing Location allocation system.
- Its identity is a client-assigned BibTeX citation key, and the citation key inside its bibliographic metadata is exactly the same identity string.
- A Citation key must never be parseable as a valid Location.
- Creation rejects a Citation key that is a valid Location.
- A Citation key must match `^[A-Za-z][A-Za-z0-9_-]{0,127}$`.
- Citation keys matching Windows reserved filename stems such as `CON`, `PRN`, `AUX`, `NUL`, `COM1` through `COM9`, or `LPT1` through `LPT9` are rejected case-insensitively.
- `go <target>` and `[[target]]` keep one shared target syntax: a valid Location resolves a Topic or Regular Card, and every other accepted target string resolves a Literature Card by Citation key.
- Its bibliographic metadata is stored as a BibTeX string in SQLite and is treated as Card metadata rather than Card body text.
- One Literature Card stores exactly one complete BibTeX entry.
- The BibTeX entry must contain a nonempty `title` field.
- The submitted BibTeX string is stored unchanged.
- ZT parses the stored entry to extract its entry key and `title` while preserving the original BibTeX string.
- The client assigns the Citation key by entering it as the entry key in the BibTeX metadata; creation has no second Citation-key input to compare against it.
- BibTeX metadata is never scanned for Link macros; only the Card body contributes outbound Links.
- Add `cards.is_lit`: `1` for a Literature Card and `0` for a Topic Card or Regular Card.
- Topic Cards and Literature Cards are distinct kinds; a Literature Card is not a Topic Card.
- A Literature Card has the same three-part Card text model: title, client-edited body, and system-managed Reverse links.
- The Literature Card title is generated from the work title in its bibliographic metadata rather than independently authored.
- The Literature Card body is edited by the client.
- The Literature Card Reverse link section is generated and repaired by ZT in the same manner as other Cards.
- Regular Cards and Literature Cards can refer to Literature Cards.
- Topic Cards can refer to Literature Cards.
- Topic Cards may also refer to Topic Cards and Regular Cards by Location.
- When saving a Topic Card, a newly introduced Link is accepted if its target resolves either to an existing Citation key or to an existing Location; a target resolving to neither is refused.
- Referring to a Literature Card should otherwise behave like referring to an existing Card: validation, rendering, activation, navigation, Broken link handling, and Reverse link generation all apply.
- `zt dp` includes Literature Cards, gives each one a 10-character-hash Markdown filename, and adds a `citation-key -> hash-filename` entry to `mapping.json`.
- A dumped Literature Card's hashed Markdown file contains both the exact stored BibTeX string and the exact stored three-section Card text in a deterministic envelope.
- A Literature Card Markdown filename is derived from SHA-256 over `Citation key`, one NUL byte, the exact BibTeX string, one NUL byte, and the exact Card text, truncated to 10 hexadecimal characters before `.md`; existing collision handling still applies.
- The dump also contains one `.bib` file per Literature Card.
- Each `.bib` file contains exactly that Literature Card's stored BibTeX string.
- Each `.bib` filename is `<citation-key>.bib`; its stem is exactly the Citation key, with no hash, prefix, or rewriting.
- Literature Card `.bib` files are additional archive entries; `mapping.json` continues to map the Citation key only to the hashed Markdown filename.
- Dump success counts Cards, not archive files; each Literature Card contributes one to `dumped <count> cards` even though it produces both Markdown and `.bib` entries.
- The destructive full-storage reset includes Literature Cards.
- Session `go <target>` can navigate to a Literature Card.
- Shell `zt l` creates a Literature Card and accepts no arguments or configuration options.
- Bare Session `l` creates a Literature Card and accepts no arguments.
- Literature Card creation first opens a dedicated BibTeX metadata editor in which the client enters one complete BibTeX entry string.
- Saving the metadata editor validates the BibTeX structure, entry count, Citation key, required `title`, and Citation-key filename rules.
- Invalid BibTeX metadata is refused and is not written to SQLite.
- After a valid metadata save, creation enters the existing Regular Card-text creation process with the title section automatically filled from the BibTeX `title` field.
- The client edits the Literature Card body through that Regular Card-text process; ZT owns the generated Reverse link section.
- Running `e` on a Literature Card first shows a numbered choice between editing BibTeX metadata and editing main Card text.
- Choosing metadata opens only the BibTeX metadata editor. A successful metadata save returns directly to the normal Literature Card view without entering main-text editing.
- Choosing main text uses the same editing and save behavior as editing a Regular Card.
- Saving changed BibTeX metadata automatically replaces the Literature Card title section with the current BibTeX `title` field.
- The Citation key is editable through the BibTeX metadata editor.
- If a metadata edit changes the BibTeX entry key, ZT renames the Literature Card to the new Citation key and rewrites every Link targeting the old Citation key so those Links remain valid.
- Citation-key rename, Link rewriting, title regeneration, and Reverse link regeneration succeed or fail as one SQLite transaction.
- Before a Citation-key rename, ZT validates the new key and refuses the save if the key is invalid or already belongs to another Literature Card.
- A refused Citation-key rename remains in the metadata editor with the client's content intact and stores none of the invalid metadata.
- A valid Citation-key rename shows the old-to-new key mapping and the number of Link macros that will be rewritten.
- The client must type `move` to confirm a Citation-key rename, matching the existing Regular Card move confirmation word.
- Session metadata validation failures remain in the same metadata-edit buffer and show the validation error.
- Shell metadata validation failures show the error and reopen `$EDITOR` with the same invalid content so the client can repair it.
- Canceling either creation stage creates no Literature Card.
- Canceling metadata editing for an existing Literature Card leaves its Citation key, BibTeX metadata, title, text, and Links unchanged.
- Invalid BibTeX metadata is never stored, either during creation or editing.
- The existing global edit lock is held across the complete two-stage Literature creation flow and across Literature metadata validation, optional Citation-key rename confirmation, and commit.

## Locked SQLite Shape

- Keep one `cards` table; do not add a second Literature Card table or structural Link table.
- Add an internal `row_id INTEGER PRIMARY KEY` used only for SQLite row identity and never shown as a Card address.
- Change `location` to a nullable unique column used only by Topic Cards and Regular Cards.
- Add a nullable `citation_key` column used only by Literature Cards, with exact-case lookup semantics.
- Add `is_lit INTEGER NOT NULL` and retain `is_topic INTEGER NOT NULL`.
- Add nullable `bibtex TEXT`; it is non-null only for Literature Cards.
- Retain `text TEXT NOT NULL` for the three-section Card text of every Card kind.
- Database constraints allow only these simple states:
  - Topic or Regular Card: `is_lit = 0`, non-null `location`, null `citation_key`, and null `bibtex`.
  - Literature Card: `is_lit = 1`, `is_topic = 0`, null `location`, non-null `citation_key`, and non-null `bibtex`.
- Unique constraints independently protect Location and Citation-key uniqueness.
- A separate ASCII case-insensitive unique index on `citation_key` rejects case-only duplicates while preserving the exact stored spelling.
- The schema deliberately avoids a second generic address column or duplicated client-visible identifier.

## Locked Navigation, Listing, and Topology

- The automatic ROOT view continues to show Topic Cards only; Literature Cards are not rendered there.
- Running `ls` while the Pointer is on ROOT lists Topic Cards and Literature Cards.
- ROOT `ls` entries for Literature Cards show Citation key and generated title.
- Running `ls` while the Pointer is inside a Topic remains limited to that Topic's Location tree and does not append the Literature Card collection.
- `stats` adds `literature: <count>`.
- `stats` continues to report `total`, `topics`, and `regular`, but `regular` excludes both Topic Cards and Literature Cards.
- The `cards` count in `zt status` includes Topic, Regular, and Literature Cards.
- A Literature Card has no direct successor, side successor, parent Location, or Topic membership.
- Session `n`, `b`, and `mv` are refused while the Pointer is on a Literature Card.
- Passing a Citation key to shell `zt n --at <location>`, `zt b --at <location>`, or `zt mv --at <location> <new-location>` is refused.
- `del` on a Literature Card deletes only that Literature Card and has no subtree or compaction behavior.
- Literature Card deletion requires typing `delete`.
- Deleting a Literature Card leaves inbound Link macros in source Cards as Broken links and regenerates Reverse links for the remaining Cards.
- After Session deletion of a Literature Card, the Pointer returns to ROOT.
- Citation-key changes are performed only through Literature metadata editing, not through `mv`.

## Locked Reading and Broken-Link Behavior

- A Literature Card reading view shows, in order: Citation key, generated title, a `metadata:` heading with the exact raw stored BibTeX entry, client-authored Card body, and generated Reverse links.
- The first version does not add a second formatted bibliography rendering; the raw BibTeX entry is the metadata presentation.
- Links to Literature Cards use the same valid, Broken, and clickable rendering as Location Links.
- A Link whose Citation key no longer exists is a Broken link.
- `go <citation-key>` and mouse activation refuse a Broken Citation-key Link and leave the Pointer unchanged.
- `lsbk` includes Broken Citation-key Links using the same source-address, source-title, broken-target, and source-line information as Broken Location Links.
- An already-Broken Citation-key Link may remain during an unrelated edit when the edit introduces no new Broken link.
- Saving rejects a newly introduced Link to a missing Citation key.
- Broken Citation-key Links do not create Reverse links because their target Literature Card does not exist.

## Locked Clear Behavior

- `zt clear` is the only full-storage deletion command; `zt clean` does not exist and is not an alias.
- `zt clear` remains shell-only and retains its existing edit-lock, open-Session refusal, exact `clear` confirmation, transaction, configuration-preservation, and dump-preservation rules.
- The clear transaction deletes Topic Cards, Regular Cards, and Literature Cards together and resets Topic allocation to `0`.
- The successful clear count includes all three Card kinds.
- After clear, `stats` reports zero total, Topic, Regular, and Literature Cards.

## Locked BibTeX Interpretation

- Use a real BibTeX parser rather than regular expressions or a hand-written partial parser.
- Metadata must contain exactly one ordinary bibliographic entry of any entry type.
- `@comment`, `@preamble`, and `@string` are not Literature Card entries and are rejected.
- The entry must have one valid Citation key and a literal, nonempty `title` field; unresolved string macros are not accepted as the title.
- BibTeX field names are interpreted case-insensitively.
- To generate the Card title, ZT removes BibTeX grouping braces, trims surrounding whitespace, and collapses internal whitespace and newlines to single spaces.
- Unicode and LaTeX command text are preserved during title normalization.
- The normalized title must be one nonempty display line.
- Title normalization never rewrites the raw BibTeX string stored in SQLite or dumped to `.bib`.

## Locked Literature Markdown Envelope

- A hashed Literature Card Markdown file uses YAML front matter followed immediately by the exact stored three-section Card text.
- The YAML front matter contains exactly `zt_kind: literature`, `citation_key: <citation-key>`, and one literal-block `bibtex` value.
- The YAML serializer uses the appropriate literal-block chomping indicator (`|-`, `|`, or `|+`) so parsing the scalar reproduces the exact BibTeX trailing-newline state.
- Every BibTeX line is indented as YAML literal-block content; YAML parsing removes that envelope indentation and recovers the exact stored BibTeX string.
- The closing `---` is followed by the Card text without adding, removing, or normalizing bytes inside `cards.text`.
- The separate `<citation-key>.bib` entry remains the authoritative literal-byte copy of the BibTeX string.
- The 10-character Markdown hash continues to use the original Citation key, raw BibTeX string, and raw Card text rather than the serialized YAML bytes.

## Locked Schema Compatibility Boundary

- Released ZT code contains no general schema-migration framework and does not automatically transform legacy databases.
- `zt up` detects the legacy `location/is_topic/text` schema and fails clearly without modifying or deleting the database.
- This implementation task nevertheless includes a one-time manual migration of the currently configured live `zt.sqlite3` file to the locked schema.
- That one-time operation is an implementation/deployment step, not a user-facing ZT command and not reusable migration code shipped in the binary.
- The manual migration must preserve all existing Topic and Regular Card Locations, flags, and exact Card text, plus the existing `metadata.next_topic_id` value.
- The manual migration creates no Literature Card rows; Literature Cards are added only through `zt l` after the upgraded binary is installed.
- The future coding agent implementing this feature owns and performs the one-time migration; the client is not asked to run raw migration commands manually.
- Before migration, the coding agent stops ZT and verifies that no Session or edit lock remains.
- The coding agent creates a consistent timestamped SQLite backup before changing the live database and retains it after successful completion.
- The schema replacement and row copy run inside one SQLite transaction.
- Before commit, the coding agent compares the count and content hash of every existing `location`, `is_topic`, and exact `text` value and verifies `metadata.next_topic_id` is unchanged.
- After commit, the coding agent runs `PRAGMA quick_check`, starts the upgraded ZT binary, and verifies `stats` against the migrated row counts.
- Any failed verification aborts the migration or restores the retained backup; an unverified migrated database is never declared complete.

## Locked Literature Body and Generated Title

- A Literature Card body may link to any existing Topic Card, Regular Card, or Literature Card using the target's Location or Citation key.
- Literature Card body validation follows the same new-Broken-Link and existing-Broken-Link rules as other link-capable Card bodies.
- The title section shown in the Literature main-text editor is generated content, not independent client-authored data.
- On every Literature main-text save, ZT replaces the title section with the current normalized BibTeX `title`, regardless of edits made to that section.
- On every Literature main-text save, ZT also replaces the Reverse link section with regenerated content.
- A metadata edit that changes the BibTeX `title` updates the Literature Card title and regenerates all Reverse link text so references that display the source title remain current.

## Locked Literature Listing and Creation Completion

- Running `ls` while the Pointer is on a Literature Card lists the complete Literature Card collection and no Topic-tree Cards.
- Literature collection entries show Citation key and generated title in Citation-key order.
- After successful Session `l` creation, the Pointer moves to the newly created Literature Card and its Literature Card reading view is rendered.
- After successful shell `zt l` creation, stdout prints only the new Citation key followed by a newline.
- Canceled creation prints no Citation key and leaves storage unchanged.

## Locked Edit Selection and Temporary Files

- Session `e` on a Literature Card shows this numbered prompt:

  ```text
  edit literature card:
    1. metadata
    2. main text
  select edit:
  ```

- The Session prompt accepts only `1` or `2`; other input reports `invalid edit option` and prompts again without changing storage.
- Session metadata editing uses the internal editor with a `metadata edit` header.
- Shell metadata editing uses `$EDITOR` with a `.bib` temporary-file suffix.
- Shell Literature main-text editing uses `$EDITOR` with the existing `.zt.md` temporary-file suffix.
- Shell `zt e --at <citation-key>` does not show the numbered prompt; it requires an explicit command-line option selecting metadata or main text.
- If the required Literature edit-selection option is absent, shell `zt e --at <citation-key>` fails before opening an editor.
- Shell `zt e --at <location>` retains the existing direct main-text edit behavior and does not require a Literature edit-selection option.

## Locked Dump Ordering

- `mapping.json` keys are ordered lexicographically across the combined set of Locations and Citation keys.
- Archive entries are ordered as the top-level `mapping.json`, then all hashed Markdown files in mapping-key order, then all `<citation-key>.bib` files in Citation-key order.
- Literature Markdown and `.bib` ordering uses the stored Citation key, not `row_id` or creation time.
- Dumping identical stored data produces the same mapping-key and archive-entry order, subject only to the existing rare hash-collision salt rule.

## Locked Shell Target Boundary

- Shell `zt e --at <target>` and `zt del --at <target>` resolve `<target>` as either a Location or Citation key.
- Shell `zt n --at <location>`, `zt b --at <location>`, and `zt mv --at <location> <new-location>` remain Location-only commands.
- No `--lit`, address prefix, or second targeting mechanism is added.
- Shell help adds argument-free `zt l` and uses `<target>` for `e` and `del`.
- Session help adds bare `l` and presents navigation as `go [<target>] | root`; a provided target may be a Location or Citation key.
- Session `e` continues to use the Pointer and the numbered choice rather than a command-line option.

## Locked Shell Literature Edit Option

- The exact shell forms are `zt e --at <citation-key> --part metadata` and `zt e --at <citation-key> --part text`.
- `--part` accepts only the literal values `metadata` and `text`.
- A Literature target without `--part`, with an unknown `--part` value, or with duplicate/conflicting edit selectors fails before opening `$EDITOR`.
- Supplying `--part` when `<target>` is a Location fails; Topic and Regular Card shell editing retains `zt e --at <location>`.
- Shell help lists the Location and Literature forms separately so the conditional requirement is explicit.
- The option is called an edit-selection option, not configuration; persistent `zt config` behavior is unchanged.

## Locked Citation-Key Case Behavior

- Citation keys preserve their exact submitted ASCII case in SQLite, BibTeX, views, Links, mappings, and `.bib` filenames.
- Target resolution is case-sensitive: `[[Smith2024]]` does not resolve a stored `smith2024` Literature Card.
- Creation and Citation-key rename reject a new key when any existing key matches it case-insensitively.
- This prevents case-only `<citation-key>.bib` filename collisions on Windows without silently changing Link identity.

## Acceptance and Verification

- Unit tests cover BibTeX parsing and rejection, Citation-key grammar and reserved names, case-only collisions, title normalization, YAML round-tripping including trailing-newline cases, hash inputs, schema constraints, and Location/Citation-key target resolution.
- CLI integration tests cover Literature creation and cancellation, metadata retry, both `--part` paths, missing/invalid/inapplicable `--part`, title repair, Citation-key rename confirmation and Link rewriting across all Card kinds, collision refusal, deletion, Broken links, `lsbk`, listing, `stats`, `zt clear`, dump contents and ordering, exact `.bib` bytes, and legacy-schema refusal.
- PTY tests cover real Session `l`, invalid metadata retry, the numbered `e` choice, metadata and main-text edits, `go`, clickable Literature Links, Citation-key rename confirmation, listing, deletion, and Pointer outcomes.
- Regression tests preserve existing Topic and Regular Card creation, editing, linking, move/delete compaction, dumping, clearing, line-session fallback, and TUI behavior.
- The coding agent records evidence for the live database backup, pre/post row hashes, preserved `next_topic_id`, `PRAGMA quick_check`, installed-binary `stats`, and retained backup path.
- Acceptance requires `cargo check`, the full `cargo test` suite, and one manual local TUI smoke run covering Literature creation, navigation, both edit choices, rename, dump, and deletion.

## KISS Implementation Boundary

- Keep one `cards` table and one shared Link/reverse-link engine.
- Add no second Literature storage service, structural Link table, address prefix, `zt clean` alias, general migration framework, formatted citation renderer, dump importer, or unrelated command/UI refactor.
- Reuse the existing Card parser, editor/save pipeline, transaction boundaries, edit lock, target rendering, Broken-link behavior, and dump machinery through small kind-aware extensions.
- A Literature Card is implemented as a Card with separate bibliographic metadata and Citation-key identity, not as a parallel note system.

## Open Decisions

None.
