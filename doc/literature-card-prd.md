# Literature Card PRD

Published as [#27](https://github.com/liudesheng9/zet/issues/27).

## Problem Statement

`zt` can currently address only Topic Cards and Regular Cards inside the Location tree. Literature used by those Cards has no first-class representation, no client-assigned bibliographic identity, and no place to preserve its BibTeX metadata. Users must therefore repeat citation details in ordinary Card text, cannot navigate directly by Citation key, and cannot rely on ZT to maintain Reverse links when literature is cited.

The existing SQLite, Link, Session, dump, and clear behavior is built around one Location-only identity. Literature Cards must join those workflows without entering the Location allocation system, weakening existing safety rules, or becoming a parallel note system.

## Solution

Add Literature Cards as a third Card kind. Each Literature Card has a client-assigned Citation key taken directly from one valid BibTeX entry, exact stored BibTeX metadata, and the same three-section Card text model used elsewhere: generated title, client-authored body, and generated Reverse links.

Locations and Citation keys remain independent user-facing address systems behind one shared target resolver. Existing `[[target]]`, `go`, editing, Link validation, Reverse links, Broken links, listing, statistics, deletion, dump, and clear behavior become kind-aware while preserving the established Topic and Regular Card contracts.

Creation is available as argument-free shell `zt l` and bare Session `l`. Metadata is validated before any row is stored. Shell and Session editing can update either BibTeX metadata or main text; a Citation-key change is a transactional rename that rewrites every Link to keep it valid.

Dump archives retain hashed Markdown output and add exact `<citation-key>.bib` files. The currently configured database receives one carefully verified manual migration performed by the coding agent, while released ZT code deliberately does not gain a general migration framework.

## User Stories

1. As a `zt` user, I want a Literature Card to represent one bibliographic work, so that literature is first-class in my note system.
2. As a `zt` user, I want Literature Cards outside the Topic Location tree, so that bibliography identity does not consume or distort Card Locations.
3. As a `zt` user, I want to assign each Literature Card through its BibTeX Citation key, so that ZT uses the identifier already present in my bibliography.
4. As a `zt` user, I want a Citation key never to parse as a Location, so that `go` and Link targets remain unambiguous.
5. As a Windows user, I want Citation keys restricted to filename-safe ASCII characters, so that exact `.bib` filenames are portable.
6. As a Windows user, I want reserved device names rejected as Citation keys, so that dumps cannot create invalid files.
7. As a `zt` user, I want Citation-key spelling and case preserved exactly, so that BibTeX identity is not silently rewritten.
8. As a `zt` user, I want case-only duplicate Citation keys rejected, so that distinct Cards cannot collide in a Windows dump.
9. As a `zt` user, I want exactly one complete BibTeX entry per Literature Card, so that metadata ownership is clear.
10. As a `zt` user, I want normal bibliographic entry types accepted, so that articles, books, proceedings, and other works can be represented.
11. As a `zt` user, I want BibTeX directives such as comments, preambles, and string definitions rejected as Literature Cards, so that every stored item is a work.
12. As a `zt` user, I want a literal nonempty BibTeX `title`, so that every Literature Card has a usable title.
13. As a `zt` user, I want the exact submitted BibTeX string preserved, so that formatting and metadata are not normalized away.
14. As a `zt` user, I want the Card title generated from BibTeX, so that the title cannot drift from the metadata.
15. As a multilingual user, I want Unicode preserved in generated titles, so that non-English literature names remain correct.
16. As a BibTeX user, I want LaTeX command text preserved while grouping braces and whitespace are normalized for display, so that generated titles remain meaningful.
17. As a `zt` user, I want BibTeX metadata excluded from Link scanning, so that bracket-like metadata is not mistaken for Card references.
18. As a `zt` user, I want invalid BibTeX refused before storage, so that malformed metadata never enters SQLite.
19. As a shell user, I want argument-free `zt l` to begin Literature creation, so that creation has one simple command.
20. As a shell user, I want BibTeX metadata edited in a `.bib` temporary file, so that my editor provides appropriate syntax behavior.
21. As a shell user, I want invalid metadata to reopen in `$EDITOR` with my content intact, so that I can repair it without starting over.
22. As a shell user, I want valid metadata followed by normal Card-text editing, so that I can add my own notes immediately.
23. As a `zt` user, I want a canceled creation to create nothing, so that partial Literature Cards cannot survive.
24. As a shell user, I want successful `zt l` to print only the Citation key, so that creation output is script-friendly.
25. As a Session user, I want bare `l` to create a Literature Card, so that I do not need to leave the Session.
26. As a Session user, I want invalid metadata to remain in the internal editor with a visible error, so that I can correct it safely.
27. As a Session user, I want successful `l` to move the Pointer to the new Literature Card, so that I can read it immediately.
28. As a Session user, I want Literature `e` to offer numbered metadata and main-text choices, so that the two editable parts are explicit.
29. As a Session user, I want metadata editing to return directly to the Literature Card view after save, so that I am not forced into a second edit.
30. As a Session user, I want main-text editing to behave like Regular Card editing, so that existing editing habits still work.
31. As a shell user, I want `--part metadata` to edit Literature metadata, so that noninteractive command parsing remains explicit.
32. As a shell user, I want `--part text` to edit Literature main text, so that the two edit paths cannot be confused.
33. As a shell user, I want Literature editing without a valid `--part` to fail before opening an editor, so that accidental edits are harmless.
34. As a shell user, I want `--part` rejected for Topic and Regular Card Locations, so that their existing edit syntax remains stable.
35. As a `zt` user, I want editing Literature main text to repair the generated title and Reverse links, so that generated zones remain authoritative.
36. As a `zt` user, I want editing BibTeX `title` to update the Literature Card title, so that metadata and view remain synchronized.
37. As a `zt` user, I want a Literature reading view to show Citation key, generated title, raw metadata, body, and Reverse links, so that the complete Card is inspectable.
38. As a `zt` user, I want `go <target>` to accept a Citation key, so that Literature Cards are directly navigable.
39. As a terminal user, I want rendered Literature Links clickable, so that mouse navigation behaves like Location Links.
40. As a `zt` user, I want Literature Cards omitted from the automatic ROOT display, so that ROOT remains a Topic overview.
41. As a `zt` user, I want ROOT `ls` to include Literature Cards, so that they remain discoverable without cluttering ROOT.
42. As a `zt` user, I want Topic `ls` to remain limited to its Location tree, so that Topic listings retain their existing meaning.
43. As a `zt` user, I want Literature `ls` to list the complete Literature collection, so that I can browse Citation keys together.
44. As a `zt` user, I want Literature listings ordered by Citation key, so that results are deterministic.
45. As a `zt` user, I want `stats` to report Literature Cards separately, so that counts distinguish all three Card kinds.
46. As a `zt` user, I want `regular` counts to exclude Topics and Literature Cards, so that the term remains accurate.
47. As a `zt` user, I want `zt status` Card count to include Literature Cards, so that operational totals remain complete.
48. As a Regular Card author, I want to Link to a Literature Card with `[[CitationKey]]`, so that my notes can cite works directly.
49. As a Literature Card author, I want to Link to Topic, Regular, and Literature Cards, so that literature notes participate in the whole graph.
50. As a Topic author, I want valid Location and Citation-key Links in the Topic body, so that a Topic can cite its relevant works and Cards.
51. As a `zt` user, I want new Links accepted only when the target exists, so that edits do not introduce Broken links.
52. As a `zt` user, I want Reverse links generated for Literature targets, so that a work shows every Card that cites it.
53. As a `zt` user, I want Citation-key Links rendered and activated like Location Links, so that identity kind does not change navigation behavior.
54. As a `zt` user, I want Links to deleted Literature Cards to become Broken links, so that source text is preserved after deletion.
55. As a `zt` user, I want `lsbk` to report Broken Citation-key Links, so that missing literature can be found and repaired.
56. As a `zt` user, I want existing Broken Citation-key Links allowed during unrelated edits, so that I can repair Cards incrementally.
57. As a `zt` user, I want a Citation key editable through metadata, so that imported bibliography mistakes can be corrected.
58. As a `zt` user, I want an invalid or colliding renamed key refused without storage changes, so that identity remains safe.
59. As a `zt` user, I want Citation-key rename to show the mapping and Link rewrite count, so that I understand the pending change.
60. As a `zt` user, I want to type `move` before Citation-key rename, so that an identity change is deliberate.
61. As a `zt` user, I want Citation-key rename to rewrite Links from every Card kind, so that references remain valid.
62. As a `zt` user, I want key rename, title repair, Link rewriting, and Reverse-link regeneration atomic, so that partial graph state cannot persist.
63. As a `zt` user, I want `del` on Literature to delete only that Card, so that unrelated Cards are never removed.
64. As a `zt` user, I want Literature deletion confirmed with `delete`, so that destructive behavior matches Regular Card safety.
65. As a Session user, I want the Pointer returned to ROOT after Literature deletion, so that it never points at a missing Card.
66. As a `zt` user, I want `n`, `b`, and `mv` rejected for Literature Cards, so that Location topology cannot be applied to Citation-key Cards.
67. As a `zt` user, I want `zt clear` to delete Literature Cards with all other Cards, so that full reset is complete.
68. As a `zt` user, I want only `zt clear` and no `zt clean` alias, so that destructive command vocabulary stays singular.
69. As a `zt` user, I want clear to preserve its existing Session, edit-lock, confirmation, configuration, dump, and transaction safety, so that Literature support does not weaken reset behavior.
70. As a `zt` user, I want each Literature Card represented by a 10-character-hash Markdown file in `zt dp`, so that it participates in the portable Card archive.
71. As a `zt` user, I want Literature Markdown to contain round-trippable BibTeX YAML metadata and exact Card text, so that one file represents the complete Card.
72. As a BibTeX user, I want an exact `<citation-key>.bib` file for every Literature Card, so that the archive is directly useful to bibliography tools.
73. As a `zt` user, I want `mapping.json` to map the Citation key to its hashed Markdown filename, so that Literature identity resolves through the existing mapping.
74. As a `zt` user, I want Literature hashes to include Citation key, raw BibTeX, and raw Card text, so that any meaningful change affects the filename.
75. As a `zt` user, I want deterministic mixed mapping and archive-entry ordering, so that dumps remain inspectable and testable.
76. As a `zt` user, I want each Literature Card counted once in dump success output, so that Card count is not confused with file count.
77. As a maintainer, I want one `cards` table and one shared Link engine, so that Literature remains a Card kind instead of a parallel system.
78. As a maintainer, I want separate nullable Location and Citation-key columns with constrained Card-kind states, so that the two address systems remain explicit.
79. As a maintainer, I want released ZT code to reject a legacy schema without mutating it, so that startup cannot silently damage data.
80. As the owner of the current database, I want the coding agent to back up and manually migrate it once, so that existing Cards survive the schema change.
81. As the owner of the current database, I want pre/post row hashes and `next_topic_id` compared, so that migration preservation is proven.
82. As the owner of the current database, I want `quick_check` and installed-binary `stats` verified after migration, so that the live database is known-good.
83. As a maintainer, I want unit, CLI integration, and PTY coverage at the existing seams, so that Literature behavior is protected without a new test architecture.
84. As a maintainer, I want existing Topic and Regular Card workflows regression-tested, so that the new Card kind does not change established behavior.
85. As a maintainer, I want one manual TUI smoke run after automated tests, so that real terminal rendering and interaction are verified.

## Implementation Decisions

- Keep a single `cards` table and the existing metadata table.
- Add an internal `row_id` primary key that is never exposed as a Card address.
- Store Topic and Regular identity in a nullable unique Location column and Literature identity in a nullable Citation-key column.
- Add non-null `is_lit`, retain non-null `is_topic`, add nullable raw BibTeX metadata, and retain non-null three-section Card text.
- Constrain non-Literature rows to a Location with no Citation key or BibTeX; constrain Literature rows to a Citation key and BibTeX with no Location and `is_topic = 0`.
- Protect Citation-key uniqueness case-insensitively while preserving exact-case lookup and display.
- Accept Citation keys matching `^[A-Za-z][A-Za-z0-9_-]{0,127}$`, reject valid Locations, and reject Windows reserved filename stems case-insensitively.
- Use one shared target resolver. Valid Locations address Topic or Regular Cards; exact-case Citation keys address Literature Cards.
- Keep Link syntax as `[[target]]`; add no address prefixes or separate Literature Link syntax.
- Use a real BibTeX parser. Accept exactly one ordinary entry of any type; reject comments, preambles, and string directives.
- Require a literal nonempty `title`, interpret field names case-insensitively, and preserve the raw BibTeX string unchanged.
- Generate one display-line title by removing grouping braces, trimming, and collapsing whitespace while preserving Unicode and LaTeX command text.
- Scan only Card bodies for outbound Links. Ignore BibTeX and generated Reverse-link text.
- Shell `zt l` and Session `l` accept no arguments. They validate metadata before entering Card-text creation and store nothing until both stages succeed.
- Hold the existing global edit lock across the complete two-stage creation and metadata rename transactions.
- Shell metadata uses a `.bib` `$EDITOR` file and retries invalid saves with content intact; shell main text uses the existing `.zt.md` path.
- Session metadata uses the internal editor and existing validation-error presentation.
- Session Literature `e` prompts with `1. metadata` and `2. main text`; invalid choices re-prompt.
- Shell Literature edit requires `zt e --at <citation-key> --part metadata|text`; missing, invalid, duplicate, or Location-inapplicable `--part` fails before editor launch.
- Main-text save always repairs generated title and Reverse links. Metadata save updates generated title and returns directly to the reading view.
- A changed BibTeX entry key is a Citation-key rename. Validate it, show old-to-new mapping and Link rewrite count, require `move`, then commit rename and graph rewrites atomically.
- Literature reading view shows Citation key, generated title, exact raw metadata, Card body, and Reverse links. It does not add formatted citation rendering.
- Automatic ROOT remains Topic-only. ROOT `ls` includes Topics and Literature; Topic `ls` remains Topic-local; Literature `ls` lists all Literature Cards in Citation-key order.
- Add `literature` to `stats`; define `regular` as excluding Topics and Literature; include all Card kinds in total/status counts.
- Permit Topic, Regular, and Literature bodies to target any existing Location or Citation key, while preserving existing-new versus already-Broken Link validation.
- Use the existing Reverse-link wording and regenerate all affected Reverse-link text after save, rename, delete, and clear-related writes.
- Shell `e` and `del` accept Location or Citation-key targets. Keep `n`, `b`, and `mv` Location-only.
- Literature deletion removes only that row after `delete`, leaves inbound macros Broken, regenerates remaining Reverse links, and returns a Session Pointer to ROOT.
- Keep `zt clear` as the only full reset command and extend its existing transaction/counts to Literature Cards.
- Dump regular and Topic Markdown exactly as before.
- Dump Literature Markdown with YAML front matter containing `zt_kind`, Citation key, and a literal BibTeX scalar that selects the required chomping mode to round-trip trailing newlines; append exact Card text after the closing marker.
- Hash Literature Markdown names from Citation key, NUL, raw BibTeX, NUL, and raw Card text, retaining existing 10-character SHA-256 truncation and collision handling.
- Add one exact `<citation-key>.bib` entry per Literature Card. Keep `mapping.json` as address-to-hashed-Markdown only.
- Order mapping keys lexicographically across Locations and Citation keys; write mapping first, hashed Markdown in mapping order, then `.bib` files in Citation-key order.
- Count Cards rather than archive files in dump success output.
- Ship no general migration framework. Detect and reject the legacy schema without modifying it.
- Assign the one-time live migration to the coding agent: stop service, verify no Session/edit lock, make a consistent timestamped backup, replace/copy schema in one transaction, compare content hashes and Topic allocation, run `quick_check`, start the installed binary, verify counts, and retain the backup.
- Keep existing service lifecycle, edit-lock, Session bare-command, command-bar, Location topology, compression, and clear-safety behavior unless explicitly changed above.

## Testing Decisions

- Prefer external behavior at the highest existing seam: PTY workflows for interactive Session behavior, CLI integration for shell/storage/archive behavior, and unit tests for pure parsing, normalization, serialization, hashing, constraints, and target resolution.
- Unit tests cover accepted and rejected BibTeX, exact raw preservation, Citation-key grammar, reserved names, case-only collisions, title normalization, YAML round-tripping with trailing-newline variants, hash inputs, Card-kind state constraints, and dual identity resolution.
- CLI integration tests cover both creation stages, cancellation, invalid metadata retry, `--part` validation and both edit paths, title repair, metadata title update, rename confirmation, cross-kind Link rewriting, collision refusal, deletion, Broken links, `lsbk`, counts, clear, dump contents/order, exact `.bib` bytes, and legacy-schema refusal.
- PTY tests cover Session `l`, metadata error retry, numbered edit selection, both edit paths, `go`, clickable Citation-key Links, rename confirmation, list contexts, deletion, and Pointer outcomes.
- Each vertical slice adds its own unit/integration/PTY coverage rather than deferring verification to a horizontal final test ticket.
- Regression coverage proves Topic and Regular creation, editing, Link/reverse behavior, move/delete compaction, dump, clear, line-session fallback, and TUI behavior remain stable.
- Live migration verification records the backup path, pre/post row counts and hashes, preserved `next_topic_id`, `PRAGMA quick_check`, installed-binary `stats`, and retained backup.
- Acceptance requires `cargo check`, the full `cargo test` suite, and a manual local TUI smoke covering creation, navigation, metadata and text edit, rename, dump, and deletion.

## Out of Scope

- A second Literature database or service.
- Structural Link rows.
- Citation-key prefixes or separate Link syntax.
- A formatted citation renderer.
- Importing or restoring dumps.
- A `zt clean` alias.
- A general schema-migration framework shipped in ZT.
- Location successors, side branches, or `mv` for Literature Cards.
- Search, command history, or unrelated TUI/editor expansion.
- Broad refactors outside the kind-aware changes required by Literature Cards.

## Further Notes

- The project glossary terms are `Regular Card`, `Literature Card`, `Citation key`, `Location`, `Link`, `Broken link`, `Reverse link`, `Pointer`, and `Session`.
- The locked source design is `doc/literature-card-design.md`.
- This PRD supersedes older contracts only for the explicitly listed Literature-specific schema, target, Topic-Link, count, and dump behavior.
- All implementation issues are AFK-ready and must carry the `ready-for-agent` label.
- The coding agent, not the user, performs the one-time migration of the currently configured SQLite database after implementation is ready.
