# Session Interactive Tests and Edit Caret PRD

## Problem Statement

The current `zt` Session implementation has important behavior that is not proven through real terminal interaction. Existing integration tests run through piped input and exercise the line-session fallback, but the raw terminal Session path uses crossterm and is where link activation, screen redraw, edit mode, and edit-lock interactions actually happen for an interactive user.

The TUI editor also behaves like an append-only text buffer. The edit caret cannot move with direction keys, which makes normal card editing painful, especially for multi-line cards and Chinese text. Users need nano-like edit caret movement and enough PTY-backed test coverage to trust the interactive Session workflow.

## Solution

Build a stronger Session implementation and test seam around the existing CLI-only design.

The Session code will be extracted into an independent Session module that owns Session structure, command handling, TUI rendering and input handling, and TUI edit-caret behavior. The public Session entrypoint remains small, while database, Card, Location, validation, and storage behavior stay in the existing core until a broader split is justified.

Interactive Session tests will use a cross-platform PTY harness, starting with a Windows spike using `portable-pty`. PTY tests will be the majority of new interactive coverage and will cover representative end-to-end workflows through the real terminal path. Model-level tests will exhaustively cover exact edit-buffer and edit-caret behavior.

TUI edit mode will support nano-like first-pass editing: direction-key movement, insertion at the edit caret, line joins with Backspace/Delete, Home/End, Enter, Tab, Ctrl+S save, Esc cancel, Ctrl+C edit cancel, line/column display, horizontal scrolling, minimal viewport scrolling, Chinese text, and malformed-save retry behavior.

## User Stories

1. As a `zt` user, I want the raw terminal Session path tested through a real PTY, so that interactive behavior is covered where I actually use it.
2. As a `zt` user, I want plain `zt` under a terminal to start a Session that can be verified by automated tests, so that regressions in the TUI path are caught.
3. As a `zt` user, I want the Session command bar to remain simple one-line input, so that command entry stays predictable.
4. As a `zt` user, I want the command bar to keep its current append/backspace behavior, so that edit-caret movement does not alter command entry.
5. As a `zt` user, I want the Session pointer to remain separate from the edit caret, so that card navigation and text editing are not confused.
6. As a `zt` user, I want TUI edit mode to show the full card text, so that title, body, and reverse-link sections remain visible during editing.
7. As a `zt` user, I want the edit caret to move left and right, so that I can correct text without rewriting the whole card.
8. As a `zt` user, I want the edit caret to move up and down across lines, so that I can edit multi-line cards naturally.
9. As a `zt` user, I want vertical movement to remember my desired column, so that moving through ragged lines behaves like nano.
10. As a `zt` user, I want Home and End to move within the logical text line, so that line navigation is predictable.
11. As a `zt` user, I want Backspace at the start of a line to join with the previous line, so that I can remove accidental line breaks.
12. As a `zt` user, I want Delete at the end of a line to join with the next line, so that I can remove line breaks from either side.
13. As a `zt` user, I want typed text to insert at the edit caret, so that edits happen where I am working.
14. As a `zt` user, I want Tab to insert a literal tab, so that card text can preserve intentional indentation.
15. As a `zt` user, I want Enter to split or create lines at the edit caret, so that normal multi-line editing works.
16. As a `zt` user, I want Ctrl+S to save and return to the card view, so that edit mode closes cleanly after a successful save.
17. As a `zt` user, I want Esc to cancel immediately, so that I can leave edit mode without a second confirmation.
18. As a `zt` user, I want Ctrl+C inside edit mode to cancel the edit and return to the Session view, so that interruption behavior is safe.
19. As a `zt` user, I want malformed saves to keep me in edit mode with my edit caret unchanged, so that I can fix the text without losing context.
20. As a `zt` user, I want validation errors shown under the edit-mode header, so that save failures are visible while editing.
21. As a `zt` user, I want the edit-mode header and command hints to remain visible, so that save/cancel controls are always discoverable.
22. As a `zt` user, I want edit mode to show line and column, so that I can orient myself in longer card text.
23. As a `zt` user writing Chinese cards, I want line/column display to count Unicode characters, so that Chinese text has sensible edit positions.
24. As a `zt` user writing Chinese cards, I want the visible edit caret to render in the correct screen position, so that wide characters do not cause drift.
25. As a `zt` user, I want long logical lines to scroll horizontally instead of soft-wrapping, so that Home, End, and columns stay tied to logical lines.
26. As a `zt` user, I want the edit viewport to scroll when content is larger than the terminal, so that the edit caret stays visible.
27. As a `zt` user, I want trailing spaces and blank lines preserved on save, so that my card text is not silently changed.
28. As a `zt` user, I want pasted Windows line endings normalized to the stored card text style, so that pasted text stays compatible.
29. As a `zt` user, I want rendered link activation tested through a real mouse-click path, so that clickable links remain reliable.
30. As a `zt` user, I want service disconnect behavior tested in the TUI path, so that the Session exits safely when the service disappears.
31. As a `zt` user, I want edit-lock behavior tested through the user-visible error, so that concurrent writes fail clearly.
32. As a maintainer, I want Session code in an independent module, so that the interactive implementation is easier to reason about.
33. As a maintainer, I want line-session behavior preserved, so that the piped fallback path remains stable.
34. As a maintainer, I want PTY tests to use isolated archive roots, so that failures do not leak state across workflows.
35. As a maintainer, I want PTY helpers to hide escape-sequence noise, so that tests describe behavior instead of terminal protocol details.
36. As a maintainer, I want editor model tests to cover boundary cases exhaustively, so that exact text-editing semantics are protected.
37. As a maintainer, I want PTY tests to sample representative workflows, so that real terminal behavior is covered without making the suite brittle.
38. As a maintainer, I want persistence checks for write workflows, so that UI success also proves the card text was stored.
39. As a maintainer, I want the implementation to stay on direct crossterm, so that this work does not introduce a new TUI framework.
40. As a maintainer, I want a manual smoke checklist, so that terminal rendering details still get one human verification pass.

## Implementation Decisions

- The Session implementation will have one small public entrypoint that chooses line-session or TUI-session behavior based on terminal detection.
- Both line-session and TUI-session paths belong in the Session module because they share Session state, Pointer handling, command routing, and registration behavior.
- The first refactor should move only Session behavior. Storage, Card, Location, validation, and persistence helpers should stay in the current core unless narrow visibility changes are required.
- Existing line-session behavior should remain stable except where shared validation or save code requires a narrow change.
- The TUI implementation remains direct crossterm. A new TUI framework is out of scope.
- PTY coverage will use `portable-pty` as a dev-dependency only if a Windows spike proves it can drive the raw terminal path.
- If the Windows PTY spike fails, implementation pauses until a Windows-capable PTY path is found.
- The editor model will be independent from crossterm events. Terminal key events translate into intent-named editor actions.
- The editor model stores text as lines plus edit-caret state and desired vertical column, converting to and from a single string at boundaries.
- The edit model treats one Rust `char` as one editable unit in the first pass.
- User-facing line/column display uses one-based coordinates and counts Unicode characters.
- Terminal placement and horizontal scrolling may use display-cell calculations so Chinese text renders correctly.
- Long logical lines horizontally scroll instead of soft-wrapping.
- Edit mode reserves fixed rows for header, status/error, editable viewport, and footer/command hints.
- The line/column display uses a simple format such as `Ln 1, Col 1`.
- Ctrl+S success returns directly to the Session card view. The card redraw is the save signal.
- Esc cancels immediately.
- Ctrl+C inside edit mode cancels edit mode and returns to the Session view.
- Malformed save failure keeps the current edit buffer and edit caret position.
- Validation errors display in a fixed status line under the edit-mode header.
- The command bar remains one-line append/backspace input and does not gain edit-caret movement.
- PTY helper APIs hide ANSI escape handling and expose named key helpers.
- PTY workflow tests that write data verify both terminal output and stored card text.
- Closed archive issue documents are not part of this work. This PRD and its child issues are the implementation authority.

## Testing Decisions

- Use the highest available seam for interactive behavior: PTY-backed tests through the real terminal path.
- Use model-level tests for exact edit-buffer, edit-caret, viewport, and boundary behavior.
- The first PTY spike passes only if it can start a terminal Session, observe `ROOT`, send `zt q`, and observe clean exit.
- PTY tests should use fresh isolated archive roots and service state per test.
- PTY tests should use a fixed terminal size.
- PTY tests should poll captured output for readiness rather than relying on fixed sleeps.
- PTY tests should be split into focused workflow tests rather than one long end-to-end test.
- First-pass PTY workflow tests should cover happy paths before error-path PTY tests are added.
- PTY workflow coverage must include the Session commands and workflows named in the locked design: navigation/read commands, rendered link activation, creation/editing, save, cancel, validation retry, delete, move, service disconnect, and edit-lock blocking.
- PTY tests should include Chinese text in at least one create/edit/save workflow.
- Model-level edit-caret tests should include Chinese text.
- Editor model tests should exhaustively cover boundary cases such as line starts, line ends, ragged vertical movement, long lines, trailing spaces, blank lines, tabs, and line-ending normalization.
- Existing line-session tests remain as regression coverage for the piped fallback path.
- Acceptance requires `cargo test` and one manual local TUI smoke run using the checklist in the locked design.

## Out of Scope

- Replacing crossterm with a new TUI framework.
- Adding command history or edit-caret movement to the Session command bar.
- Implementing nano features outside the locked first-pass operations, including search, cut/paste buffers, paging commands, and mouse-based edit-caret placement.
- Full grapheme-cluster editing for emoji sequences.
- Broad storage, domain, or schema refactors.
- Updating closed archive issue documents.
- Unix-only PTY coverage as a substitute for the required cross-platform approach.

## Further Notes

- This PRD is based on the locked design in `doc/session-interactive-tests-and-edit-caret-design.md`.
- The project glossary term is `Edit caret`, not edit cursor, and it is distinct from the Session `Pointer`.
- The implementation should stay KISS: small public Session entrypoint, narrow visibility changes, representative PTY workflows, exhaustive model tests, and no unrelated feature expansion.
