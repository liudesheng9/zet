# Session Mouse Selection and Operating-System Clipboard PRD

## Problem Statement

An interactive ZT Session captures the mouse inside a raw-mode alternate screen. Users can click rendered Links, but they cannot drag across Session text, copy an exact section to the operating-system clipboard, or paste clipboard text back into the Session. This affects every rendered TUI view, including `ROOT`, Card views, messages, help, modal confirmation/selection screens, and the internal Card editor.

The editor already has a character-based Edit caret and nano-like movement, but it has no logical selection range. The command bar is intentionally one-line append/backspace input and has no clipboard paste path. Plain `Ctrl+C` is already meaningful: it exits normal Session view and cancels edit mode. Clipboard support must therefore add familiar terminal behavior without breaking established Session, Pointer, Link, command-bar, or editor behavior.

## Solution

Add direct left-button drag selection throughout the TUI Session. Non-editor views select visible rendered screen text; edit mode selects the underlying logical Card text. Selected text is shown with reverse-video highlighting and copied to the operating-system clipboard only with `Ctrl+Shift+C`.

Add operating-system clipboard paste only through `Ctrl+Shift+V`. In normal Session view, paste appends sanitized text to the one-line command bar without executing it. In edit mode, paste inserts at the Edit caret or replaces the active logical selection using nano-like editing behavior.

Keep the implementation KISS. Use one synchronous text-only clipboard adapter on the existing event-loop thread, one small rendered-screen selection model for non-editor TUI views, and one logical selection range in the existing editor model. Preserve direct crossterm rendering, existing plain `Ctrl+C` behavior, the piped line-session fallback, Link activation, Card validation, and terminal teardown.

## User Stories

1. As a Session user, I want to drag across text in any non-editor TUI view, so that I can select text without leaving ZT.
2. As a Session user, I want selection to work in `ROOT`, Card, help, message, confirmation, and selection screens, so that copy is not tied to one Session state.
3. As a Session user, I want normal-view selection to represent only currently visible screen text, so that copying behaves like a terminal.
4. As a Session user, I want selected screen text highlighted with reverse video, so that the active selection is unambiguous.
5. As a Session user, I want forward and backward dragging to select the same normalized range, so that drag direction does not change copied content.
6. As a Session user, I want selection endpoints to snap to whole displayed characters, so that Chinese characters are never copied partially.
7. As a Session user, I want copied screen text to omit trailing blank terminal cells, so that terminal padding does not enter the clipboard.
8. As a Session user, I want visible tabs copied as rendered spaces outside edit mode, so that copied screen text matches what I saw.
9. As a Session user, I want real output-line boundaries copied as newlines, so that multiline Session output keeps its structure.
10. As a Session user, I want terminal-width soft wraps copied without added newlines, so that one logical output line stays one line.
11. As a Session user, I want `Ctrl+Shift+C` to copy the active selection to the operating-system clipboard, so that I can paste it into another application.
12. As a Session user, I want copying to keep the selection highlighted, so that I can see what was copied.
13. As a Session user, I want `Ctrl+Shift+C` with no selection to do nothing, so that it cannot accidentally trigger plain `Ctrl+C` behavior.
14. As a Session user, I want a new click or drag to replace the current selection, so that selection state stays simple.
15. As a Session user, I want command input, command execution, view changes, and invalidating redraws to clear normal-view selection, so that stale screen coordinates are never reused.
16. As a Session user, I want terminal resize to clear screen-based selection, so that highlighting does not drift after geometry changes.
17. As a Session user, I want a click without movement on a rendered Link to navigate normally, so that existing Link activation remains intact.
18. As a Session user, I want dragging across a rendered Link to select text without navigating, so that click and drag gestures cannot conflict.
19. As a Session user, I want `Ctrl+Shift+V` in normal Session view to append clipboard text to the command bar, so that I can reuse external text in commands.
20. As a Session user, I want command-bar paste to convert clipboard newlines and tabs to spaces, so that the command bar remains one line.
21. As a Session user, I want pasted command text to wait for `Enter`, so that paste never executes a command automatically.
22. As a Session user, I want invalid clipboard controls rejected as one operation, so that unsafe or partial command text is never inserted.
23. As a Session user, I want clipboard failure to leave the command bar and selection unchanged and show a status error, so that failure is safe and visible.
24. As an editing user, I want a single left click in editable Card text to place the Edit caret and clear selection, so that mouse placement behaves like nano.
25. As an editing user, I want direct left-button dragging to select logical Card text, so that I do not need a mark command or `Shift` modifier.
26. As an editing user, I want edit selection to preserve exact Unicode, tabs, spaces, and logical newlines, so that copied Card text is lossless.
27. As an editing user, I want editor selection highlighted with reverse video and retained after copy, so that the selection stays visible.
28. As an editing user, I want dragging at viewport edges to extend selection through event-driven vertical and horizontal scrolling, so that I can select off-screen Card text.
29. As an editing user, I want edit selection to survive terminal resize, so that logical Card positions remain selected after redraw.
30. As an editing user, I want an arrow, `Home`, or `End` to clear selection and move from the drag-release endpoint, so that caret movement remains predictable.
31. As an editing user, I want typing, `Tab`, or `Enter` to replace selected text, so that editing matches a normal nano-like editor.
32. As an editing user, I want `Backspace` or `Delete` to remove selected text, so that either deletion key handles the selection.
33. As an editing user, I want `Ctrl+Shift+V` to replace selected text and leave the Edit caret at the selection start, so that paste replacement is deterministic.
34. As an editing user, I want paste without a selection to insert at the Edit caret, so that normal paste remains useful.
35. As an editing user, I want pasted CRLF and lone-CR line endings normalized to LF, so that Card storage keeps its established line-ending style.
36. As an editing user, I want pasted Unicode, tabs, blank lines, spaces, and trailing spaces preserved, so that clipboard paste does not silently rewrite Card text.
37. As an editing user, I want pasted controls other than newline and tab rejected as one operation, so that terminal control data cannot enter the editor.
38. As an editing user, I want clipboard failure to preserve selection, buffer, and Edit caret while showing a fixed status error, so that I can continue safely.
39. As a Session user, I want copy and paste available only through `Ctrl+Shift+C` and `Ctrl+Shift+V`, so that the keyboard contract is explicit.
40. As a Session user, I want plain `Ctrl+C` to keep exiting normal Session view and canceling edit mode, and `Esc` to keep canceling edit mode, so that existing interruption behavior remains stable.
41. As a Session user, I want right-click and middle-click to perform no clipboard operation, so that copy and paste remain keyboard-only.
42. As a Session user, I want no double-click, triple-click, keyboard-selection, or alternate clipboard bindings, so that the feature stays KISS.
43. As a Session user, I accept configuring my terminal to forward `Ctrl+Shift+C` and `Ctrl+Shift+V`, so that ZT does not need fallback bindings.
44. As a piped-session user, I want the non-TTY line-session fallback unchanged, so that scripts retain their current behavior.
45. As a Linux user, I accept platform-native clipboard lifetime after the Session exits, so that ZT does not add a clipboard keeper process.
46. As a maintainer, I want one text-only cross-platform clipboard dependency, so that OS integration remains narrow.
47. As a maintainer, I want clipboard access kept outside deterministic selection models, so that automated tests never touch the user's real clipboard.
48. As a maintainer, I want raw PTY tests for mouse gestures and visible highlighting, so that the real crossterm path is covered.
49. As a maintainer, I want model tests for exact screen and Card-text selection semantics, so that Unicode and boundary behavior are exhaustive.
50. As a maintainer, I want one real Windows Terminal clipboard smoke test, so that external copy and paste are verified on the target machine.
51. As a maintainer, I want terminal raw mode, alternate-screen state, and mouse capture restored after exit, so that clipboard work cannot damage the surrounding shell.

## Implementation Decisions

- The existing TUI Session event loop remains the owner of terminal events, screen selection state, clipboard access, and redraw decisions.
- Non-editor TUI views use one small selection model over a retained representation of rendered terminal cells and real-versus-soft line boundaries.
- Every non-editor TUI renderer that captures the mouse participates in screen selection and copy, including normal Card/ROOT views and modal confirmation or selection screens.
- Edit mode stores a normalized half-open logical selection range alongside the existing Edit caret and viewport state.
- Selection uses character positions for user text and display-cell calculations only for terminal coordinates and highlighting.
- Normal-view copy serializes visible rendered characters, expands displayed tabs, omits terminal padding, preserves real output newlines, and suppresses soft-wrap newlines.
- Edit-mode copy serializes exact underlying Card text, including Unicode, tabs, spaces, and logical LF boundaries.
- Left-button press establishes an anchor, movement establishes a drag, and release completes the gesture. A no-movement click activates a Link outside edit mode or places the Edit caret inside edit mode.
- Direct dragging does not require `Shift`. Copy keeps selection highlighted. These are intentional deviations from GNU nano.
- Selected text uses reverse-video highlighting. Existing Link styling remains when a Link is not selected.
- Successful copy or paste shows no extra success message; persistent highlighting or visibly inserted text is the feedback.
- `Ctrl+Shift+C` copies only when a nonempty selection exists. It never falls through to plain `Ctrl+C`.
- `Ctrl+Shift+V` targets only the normal command bar or editable Card text. It does not paste into modal confirmation/selection inputs.
- Command-bar paste appends at the end, converts newlines and tabs to spaces, rejects other control characters atomically, and never submits the command.
- Editor paste normalizes CRLF and lone CR to LF, preserves all other allowed text exactly, rejects controls other than LF and tab atomically, and inserts at the Edit caret or replaces selection.
- Typing, `Tab`, `Enter`, `Backspace`, and `Delete` follow the locked selection-replacement rules. Replacement and deletion leave the Edit caret at the former selection start.
- Caret movement clears edit selection and then performs the requested movement from the drag-release endpoint.
- Edit edge scrolling advances one row or display column per drag event, with no timer, acceleration, or separate scrolling subsystem.
- Normal screen selection clears when rendered coordinates become invalid. Logical edit selection survives resize and recomputes its viewport highlight.
- Clipboard failures display in the existing fixed status/message area and do not partially mutate selection, buffer, Edit caret, or command bar.
- Use `arboard` 3.6 for text clipboard access, disable default image features, and enable Wayland clipboard support.
- Own one clipboard handle for the TUI Session lifetime and access it synchronously from the event-loop thread. Do not add a singleton, worker, retry loop, asynchronous layer, background keeper, or daemon integration.
- Keep direct crossterm rendering. Do not introduce a TUI framework or general terminal-emulator abstraction.
- Keep storage, Card parsing/validation, Session command routing, Link resolution, and the piped line-session fallback unchanged except for narrow integration required by the feature.
- Keep copy/paste keyboard-only. Right/middle mouse buttons, double/triple click selection, Shift-arrow selection, nano mark bindings, and fallback clipboard shortcuts are not added.
- Plain `Ctrl+C` keeps its existing normal-view exit and edit-cancel meanings, and `Esc` keeps canceling edit mode.

## Testing Decisions

- Prefer the highest existing behavioral seam: PTY workflows for raw terminal input/rendering, Session/controller tests for clipboard routing, and model tests for exact selection and text transformations.
- Automated tests use a deterministic in-memory clipboard fake and never read or modify the user's operating-system clipboard.
- Unit/model tests cover forward and backward selection, half-open boundaries, end-of-line snapping, real versus soft line boundaries, Chinese display cells, rendered tabs, omitted padding, logical tabs/newlines, resize lifecycle, selection clearing, and no-selection copy.
- Editor tests cover click-to-caret, movement from the drag endpoint, typing/Tab/Enter replacement, Backspace/Delete removal, paste insertion/replacement, caret placement, CRLF normalization, preservation of blank/trailing whitespace, invalid control rejection, clipboard failure, and edge-scrolling viewport behavior.
- PTY helpers expose named mouse press, drag, and release events plus `Ctrl+Shift+C` and `Ctrl+Shift+V` key sequences.
- PTY workflows cover non-editor dragging and highlighting in representative normal and modal views, Link click versus drag, Chinese text, command-bar paste routing without auto-execution, editor click/drag/highlighting, edge scrolling, resize behavior, and preservation of plain `Ctrl+C` and `Esc` behavior.
- Each implementation slice includes its own model/controller/PTY coverage rather than deferring tests to a horizontal cleanup issue.
- Existing Session, Link, Card editing, validation, line-session, and terminal-teardown tests remain as regression coverage.
- Acceptance requires `cargo check`, the full `cargo test` suite, and the manual Windows Terminal checklist in the locked design.
- The manual smoke verifies real external copy from normal and editor views, real paste into command bar and editor, Chinese and multiline content, Link click-versus-drag, edge scrolling, resize behavior, save/reopen fidelity, plain `Ctrl+C`, and terminal restoration.

## Out of Scope

- Clipboard support for the piped non-TTY Session.
- Paste into modal confirmation, verification, or edit-part selection inputs.
- Command-bar caret movement, selection, command history, or automatic execution after paste.
- Right-click or middle-click copy/paste.
- Double-click word selection or triple-click line selection.
- `Shift+Arrow`, GNU nano mark keys, or any other keyboard-selection mechanism.
- Alternative copy/paste shortcuts when the terminal intercepts `Ctrl+Shift+C` or `Ctrl+Shift+V`.
- Cut operations, a Session-local clipboard, clipboard history, image/HTML clipboard formats, or a Linux clipboard keeper.
- Background timers, selection acceleration, asynchronous clipboard work, or retry loops.
- Full grapheme-cluster editing beyond the existing one-Rust-`char` editing model.
- Replacing crossterm, adding a TUI framework, or performing unrelated Card/storage/command-routing refactors.

## Further Notes

- The locked source design is `doc/session-mouse-selection-and-clipboard-design.md`.
- Project vocabulary remains `Session`, `Pointer`, `Edit caret`, `Card`, and `Link`; selection is an editor/terminal interaction, not a new domain term.
- Explicit locked behavior takes precedence over GNU nano inside edit mode; GNU nano-like behavior then guides unspecified editor details. Conventional terminal behavior guides other TUI views.
- All implementation slices are AFK-ready and carry the `ready-for-agent` label.
- The implementation must stay KISS and must not broaden the locked feature while solving terminal or clipboard edge cases.
