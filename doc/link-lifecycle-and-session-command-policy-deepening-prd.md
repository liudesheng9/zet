# Link Lifecycle and Session Command Policy Deepening PRD

Status: implemented and verified through GitHub PRD [#43](https://github.com/liudesheng9/zet/issues/43) and issues #44-#49 on 2026-07-22.

## Problem Statement

ZT currently interprets the same Link text in several places. Card save validation, Link rewriting, Reverse link generation, Broken link reporting, and TUI rendering each scan or classify Link macros independently. A behavior-preserving change therefore risks fixing one path while leaving another path different.

ZT also implements Session command meaning twice: once for the raw terminal TUI and once for the piped Session fallback. Both paths repeat command parsing, preconditions, errors, Card operations, confirmations, and Pointer transitions even though only their interaction and presentation behavior genuinely differs.

These shallow interfaces make maintenance risky. Link or Session changes require broad code knowledge, high-level tests do not have a focused module interface beneath them, and repeated implementations can drift while still passing only their own path-specific tests.

## Solution

Deepen the Link lifecycle behind one crate-private module that owns Link syntax, target occurrences, validation, classification, exact rewriting, Reverse link derivation, and Broken link results. The module receives immutable Card snapshots and returns structured outcomes; it does not open SQLite or own transactions. Storage continues to apply changes atomically, while shell and TUI callers retain their existing formatting and interaction behavior.

Collapse Session command policy behind one crate-private command module with one execution entry point. That module owns bare command parsing, command meaning, preconditions, Pointer transitions, and structured results. The existing TUI and piped paths remain separate adapters because their editing, confirmation, rendering, redraw, mouse, and clipboard behavior genuinely differs.

The work is a zero-observable-behavior-change refactor. The Link module is completed first, then the Session command module consumes its structured results. Existing CLI and PTY workflows remain the highest acceptance seams, with focused tests added at the new module interfaces.

## User Stories

1. As a ZT user, I want this architecture change to preserve every existing command and result, so that maintenance work does not alter my workflow.
2. As a ZT user, I want existing errors, confirmations, output ordering, and Pointer outcomes preserved, so that scripts and habits remain stable.
3. As a maintainer, I want one interpretation of Link macro syntax, so that Link behavior cannot drift across callers.
4. As a maintainer, I want one canonical representation of Link occurrences and source ranges, so that validation, rewriting, and rendering use the same text interpretation.
5. As a maintainer, I want Link behavior requested through outcome-oriented operations, so that callers do not scan raw Card text themselves.
6. As a maintainer, I want the Link module to accept immutable Card snapshots, so that Link behavior is testable without SQLite.
7. As a maintainer, I do not want a storage trait while SQLite is the only storage implementation, so that the refactor adds no hypothetical seam.
8. As a maintainer, I want storage to retain ownership of SQLite connections and transactions, so that the Link module remains independent of persistence.
9. As a Card author, I want `[[target]]` to remain the only Link macro syntax, so that no new syntax is introduced by the refactor.
10. As a Card author, I want display text inside Link macros to remain unsupported, so that parsing behavior stays unchanged.
11. As a Card author, I want a Link target to remain a valid Location or Citation key, so that Card addressing stays unchanged.
12. As a Literature Card user, I want Citation key case to remain significant, so that exact identity is preserved.
13. As a Card author, I want only the client-authored Card body scanned for outbound Links, so that generated or descriptive text does not create relationships.
14. As a Literature Card user, I want BibTeX metadata excluded from Link scanning, so that bibliographic text remains literal.
15. As a Card author, I want Card titles excluded from outbound Link scanning, so that title text is not treated as a relationship.
16. As a Card author, I want generated Reverse link sections excluded from outbound Link scanning, so that generated text cannot recursively create Links.
17. As a Card author, I want malformed Link writes rejected with the existing errors, so that invalid Card text is not persisted.
18. As a reader of manually malformed stored Card text, I want the body still shown without malformed fragments becoming clickable, so that read access remains useful and safe.
19. As a maintainer, I want strict Link operations on malformed stored text to fail without writes, so that corruption cannot cause partial mutations.
20. As a maintainer, I do not want automatic Link repair, so that a refactor cannot silently rewrite client-authored text.
21. As a Card author, I want a newly introduced Link to a missing target rejected, so that edits do not introduce a new Broken link.
22. As a Card author, I want an already-Broken target to remain allowed during an unrelated edit, so that I can repair other Card text without first restoring the target.
23. As a maintainer, I want "new Broken link" to retain target-set semantics, so that occurrence-count behavior does not change accidentally.
24. As a reader, I want a valid Link highlighted and clickable in the TUI, so that navigation remains unchanged.
25. As a reader, I want a Broken link visibly Broken and non-clickable, so that failed navigation remains safe.
26. As a TUI user, I want terminal geometry, wrapping, styling, and mouse hit testing preserved by the TUI adapter, so that the Link module does not absorb terminal behavior.
27. As a Card author, I want repeated Links from one source Card to produce one Reverse link line, so that generated references remain deduplicated.
28. As a Card author, I want a Broken link to produce no Reverse link line, so that missing targets are not represented as live relationships.
29. As a Card author, I want deletion of a target to preserve inbound Link macros as Broken links, so that source text is not destroyed.
30. As a Card author, I want Reverse link wording and source ordering preserved exactly, so that generated Card text remains stable.
31. As a maintainer, I want Reverse link sections derived by the Link module, so that every mutation uses one relationship interpretation.
32. As a maintainer, I want Reverse link regeneration to remain inside the caller's atomic transaction, so that partial generated state cannot persist.
33. As a user moving Regular Cards, I want only actual Link targets rewritten, so that identical plain text is not changed.
34. As a user renaming a Citation key, I want only actual Link targets rewritten, so that titles and BibTeX metadata remain untouched.
35. As a Card author, I want rewriting to preserve every byte outside the target range, so that punctuation, whitespace, and line endings remain exact.
36. As a user previewing a move or Citation-key rename, I want rewrite counts to remain occurrence counts, so that confirmation output stays unchanged.
37. As a shell user, I want `zt lsbk` to format structured Broken link results with its current output, so that audits remain stable.
38. As a TUI Session user, I want `lsbk` to use the same Broken link results while retaining its compact message format, so that both paths agree on data without losing their presentation.
39. As a maintainer, I want Broken link results to retain the source address, source title, target, and source line, so that all current reporting remains possible.
40. As a maintainer, I want one production Link scanner after migration, so that the old shallow implementations are actually deleted.
41. As a maintainer, I want focused Link tests for syntax, ranges, validation, rewriting, Reverse links, and Broken links, so that the module interface is the test surface.
42. As a maintainer, I want existing CLI and PTY Link workflows retained, so that focused tests do not replace end-to-end proof.
43. As a Session user, I want TUI and piped command input to retain identical command meaning, so that the supported Session paths cannot drift.
44. As a maintainer, I want one Session command execution entry point, so that parsing, preconditions, operations, and Pointer transitions are concentrated.
45. As a maintainer, I want one Session state owner for Pointer changes, so that adapters cannot assign divergent outcomes.
46. As a maintainer, I want the TUI and piped paths retained as real adapters, so that genuine interaction differences stay local.
47. As a maintainer, I want the interaction seam limited to editing, edit-part choice, confirmation, and presentation, so that it does not become one shallow method per command.
48. As a Session user, I want bare commands to remain free of an executable prefix, so that command-bar syntax stays unchanged.
49. As a Session user, I want leading and trailing whitespace trimmed and blank input preserved as a no-op, so that input behavior stays unchanged.
50. As a Session user, I want `t <title>` to retain its current non-shell quote behavior, so that title parsing does not change.
51. As a Session user, I want unknown commands to retain `unknown session command: <first-token>`, so that errors remain precise.
52. As a Session user, I want extra `go` arguments to retain `usage: go [<target>]`, so that malformed navigation behaves identically.
53. As a Session user, I want bare `go` and `root` to return the Pointer to `ROOT`, so that root navigation remains unchanged.
54. As a Session user, I want `go <Location>` to retain successful Location navigation, so that Topic and Regular Card workflows remain stable.
55. As a Session user, I want `go <Citation-key>` to retain successful Literature Card navigation, so that Literature workflows remain stable.
56. As a Session user, I want failed navigation to leave the Pointer unchanged, so that errors remain safe.
57. As a shell user, I want `zt go` to remain unavailable, so that a shell process does not gain implicit Pointer behavior.
58. As a Session user, I want the current Session command set and help text preserved, so that discoverability remains stable.
59. As a Session user at `ROOT`, a Location, or a Citation key, I want current command preconditions preserved, so that invalid operations fail in the same contexts.
60. As a Session user, I want a failed command to leave the Pointer unchanged, so that command errors cannot move my view.
61. As a Session user, I want a canceled edit or confirmation to preserve the Pointer and Card data, so that cancellation remains safe.
62. As a Session user creating a Card, I want the current successful Pointer destination preserved, so that creation workflows remain continuous.
63. As a Session user editing a Card, I want the current Pointer destination preserved, including Citation-key rename outcomes, so that editing remains predictable.
64. As a Session user deleting a Card, I want the current parent-or-`ROOT` Pointer outcome preserved, so that deletion navigation remains stable.
65. As a Session user moving a Card, I want the Pointer to follow the moved Card, so that move behavior remains stable.
66. As a Session user, I want `q`, Ctrl+C, and disconnect behavior preserved, so that Session lifecycle remains unchanged.
67. As a maintainer, I want read-only commands to return structured semantic results, so that adapters format data without duplicating command meaning.
68. As a TUI user, I want `ls`, `stats`, `status`, `lsbk`, and `help` to retain compact TUI formatting, so that the message area remains usable.
69. As a piped Session user, I want read-only commands to retain their current line-oriented formatting, so that scripted output remains stable.
70. As a TUI Session user, I want successful commands to clear or retain the status message exactly as today, so that redraw behavior does not drift.
71. As a piped Session user, I want the current view printed after successful commands and not printed after command errors, so that line-session output remains compatible.
72. As a TUI editor user, I want `EditorModel`, mouse, clipboard, save, and cancel behavior unchanged, so that command consolidation does not alter editing.
73. As a piped Session user, I want the existing byte-oriented edit protocol unchanged, so that the fallback path remains compatible.
74. As a TUI user, I want confirmation modals to retain their mouse-selection and clipboard behavior, so that destructive prompts remain usable.
75. As a piped Session user, I want line-oriented confirmations unchanged, so that delete and move prompts remain scriptable.
76. As a Literature Card user, I want Citation-key rename confirmation preserved through the shared interaction seam, so that metadata edits remain safe.
77. As a maintainer, I want edit locks, validation, Card mutations, and SQLite transactions preserved, so that command consolidation changes ownership rather than behavior.
78. As a shell user, I want shell parsing and shell output unchanged, so that Session refactoring remains Session-only.
79. As a maintainer, I want shell and Session callers to share data-returning queries where useful, so that direct-print functions do not force duplicate Session policy.
80. As a maintainer, I do not want Pointer state added to shell commands, so that the shell and Session contracts remain distinct.
81. As a maintainer, I want the Link module completed before Session command consolidation, so that `lsbk` and Link results have one owner before Session policy moves.
82. As a maintainer, I want behavior characterized before ownership moves, so that an architecture refactor cannot hide a semantic change.
83. As a maintainer, I want each migration slice green before the next one starts, so that failures remain local and reviewable.
84. As a maintainer, I want old scanners and duplicated command handlers deleted only after all consumers move, so that the deletion test proves real depth.
85. As a maintainer, I want focused tests at both new module interfaces, so that most behavior can be verified without terminal or process setup.
86. As a maintainer, I want process-level CLI and piped Session tests retained, so that real command, storage, and output behavior remains proven.
87. As a maintainer, I want PTY-backed TUI tests retained, so that raw mode, rendering, mouse, clipboard, edit, and confirmation paths remain proven.
88. As a maintainer, I want repository-wide searches to prove no duplicate production Link scanner or Session command match remains, so that completion is evidence-backed.
89. As a maintainer, I want formatting, all-target compilation, the full test suite, strict linting, and whitespace checks to pass, so that the refactor meets the complete Rust quality gate.
90. As a maintainer, I want active documentation consistent with the new ownership, so that future work does not restore the shallow architecture.
91. As a maintainer, I want no schema, new Link syntax, new command, configuration, recovery state, or telemetry change, so that this work remains an architecture refactor.
92. As a maintainer, I want no broad Location topology, Card mutation, Session reading-projection, or TUI framework refactor, so that the approved scope stays bounded.

## Implementation Decisions

- Implement one crate-private Link module as the sole production owner of Link macro interpretation.
- Use one canonical Card-body representation for Link occurrences, exact target ranges, validation, classification, and rewriting.
- Add a cross-Card Link lifecycle representation built from immutable Card snapshots containing only Card address, title, and body.
- Keep SQLite, edit-lock, transaction, schema, and mutation orchestration outside the Link module.
- Introduce no storage trait while SQLite is the only storage implementation.
- Return structured Broken link results and Reverse link sections for callers to format or persist.
- Preserve exact body-only rewriting, occurrence counts, target-set Broken-link comparison, Reverse link wording, and deterministic ordering.
- Keep TUI terminal geometry, styling, wrapping, and mouse hit spans inside the TUI adapter.
- Preserve useful read access for malformed stored Link text, reject malformed writes, fail strict Link operations without writes, and add no automatic repair.
- Migrate every Link consumer before deleting the old scanners; raw Link scanning outside the Link module is prohibited afterward.
- Implement one crate-private Session command module with one execution entry point.
- Give the Session command module ownership of bare input parsing, command matching, arity, preconditions, Card operation selection, confirmation requirements, Pointer transitions, continuation, exit, and structured results.
- Retain TUI and piped Sessions as separate adapters at one real interaction seam.
- Limit the interaction seam to genuinely varying editing, Literature edit selection, confirmation, and result presentation behavior; do not create one adapter method per command.
- Keep crossterm events, TUI drawing, `EditorModel`, mouse, clipboard, piped input, piped output, and the byte-oriented piped editor inside their adapters.
- Keep shell command dispatch, syntax, output, and lack of Pointer state separate and unchanged.
- Separate shared data retrieval from direct formatting where current Session code calls shell-printing functions.
- Implement the Link deepening before Session command consolidation.
- Move ownership incrementally: characterize, migrate Link consumers, migrate Session command classes, then delete old implementations.
- Treat every observed behavior difference as a blocking design question rather than choosing new behavior during refactoring.

## Testing Decisions

- Prefer focused tests at the new Link and Session command interfaces for most behavior.
- Link interface tests use immutable Card snapshots and cover syntax, source ranges, multiple occurrences, target grammar, valid/Broken classification, old-versus-new Broken targets, exact rewriting, occurrence counts, Reverse link derivation, Broken link results, ordering, wording, and malformed-text behavior.
- Session command interface tests use scripted TUI-like and piped-like adapters and cover every command, arity, context precondition, Pointer outcome, cancellation, confirmation, structured result, formatting choice, and redraw decision.
- Keep process-level CLI tests for shell behavior, SQLite transactions, exact output, Link rewriting, Reverse links, Broken links, and shell separation.
- Keep process-level piped Session tests for bare input, output formatting, redraw, edit protocol, confirmations, errors, and Pointer outcomes.
- Keep PTY-backed tests for real TUI input, raw mode, rendering, Link activation, mouse, clipboard, edit mode, confirmations, message clearing, and disconnect behavior.
- Every vertical slice includes its own focused and end-to-end proof; there is no horizontal test-only issue.
- Characterization tests are added before replacing a current scanner or command path.
- The final gate is `cargo fmt -- --check`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, and repository-wide duplicate-implementation searches.

## Out of Scope

- New Link syntax or Link display text.
- A structural Link table or cached Link index in SQLite.
- Schema migration.
- A new storage trait.
- Location topology changes.
- A broad Card mutation refactor.
- A broad Session reading-projection refactor beyond structured command results.
- Merging the TUI and piped event loops.
- A new TUI framework.
- Shell command or shell output changes.
- New commands, flags, configuration, recovery state, or telemetry.
- User-facing wording, formatting, or Pointer behavior changes.
- Version bump, installation, implementation closeout, or release work as part of planning.

## Further Notes

- The locked source design is `doc/link-lifecycle-and-session-command-policy-deepening-design.md`.
- The canonical project terms are Card, Literature Card, Citation key, Location, Pointer, Session, Link, Broken link, and Reverse link.
- This PRD supersedes only the old internal instruction to retain two duplicated Session command handlers. It preserves every user-visible Session contract.
- The approved implementation plan uses six AFK tracer-bullet issues in dependency order: three Link slices followed by three Session slices.
- All six implementation issues and this PRD must carry `ready-for-agent` and `enhancement`.
- No implementation is included in this PRD and issue-publication work.
