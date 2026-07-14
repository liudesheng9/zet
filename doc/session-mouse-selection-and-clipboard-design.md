# Session Mouse Selection and Clipboard Design

Status: design locked for implementation.

## Goal

Add mouse-drag text selection throughout the interactive terminal Session and make copy and paste available only through `Ctrl+Shift+C` and `Ctrl+Shift+V`.

Keep the feature KISS: implement only the locked gestures and semantics, without expanding into a general terminal or editor framework.

## Grounded Facts

- ZT is a local terminal note system built around Cards, Locations, a per-Session Pointer, and an edit caret inside TUI edit mode.
- The interactive Session uses crossterm raw mode, enters the alternate screen, and enables mouse capture.
- In the normal Session card view, the current mouse behavior handles only a left-button press on a rendered Link; it moves the Session Pointer to the Link target.
- The normal Session command bar is intentionally one-line append/backspace input. It does not have caret movement or selection.
- TUI edit mode has an `EditorModel` backed by logical text lines, a character-based edit-caret position, a desired vertical column, and viewport offsets.
- TUI edit mode translates crossterm key events into editor intents and actions. It currently ignores mouse events.
- User-facing edit positions count Unicode characters. Horizontal viewport placement separately counts terminal display cells so Chinese text renders correctly.
- The current editor has no selection anchor, selection range, selected-text extraction, replacement-on-paste behavior, or clipboard abstraction.
- `Ctrl+C` currently has two established meanings:
  - in normal Session view, it exits the Session;
  - in TUI edit mode, it cancels the edit and returns to the Session card view without saving.
- `Esc` also cancels TUI edit mode, so edit cancellation already has a second key.
- `Ctrl+V` has no explicit Session or editor behavior today.
- Current PTY support can send key sequences and a left click, but it has no drag helper yet.
- The existing locked editor design explicitly deferred cut/paste buffers and mouse-based caret placement; this feature reopens those two deferred areas.
- The piped non-TTY Session is a separate fallback path and has no mouse input.
- The current [GNU nano manual](https://www.nano-editor.org/dist/latest/nano.html) says that a mouse click places the edit cursor, a second click at the same position toggles the mark, and terminal-native left-button selection uses `Shift` while dragging.
- GNU nano clears its marked selection after copying it, and unmodified caret movement cancels a Shift-created selection.
- Those nano behaviors conflict with some already locked ZT requirements, so any intentional deviations must be stated explicitly.
- Current [`arboard` 3.6.1 documentation](https://docs.rs/arboard/latest/arboard/struct.Clipboard.html) provides operating-system-independent `get_text` and `set_text` operations for Windows, macOS, and Linux.
- `arboard` documents that Windows clipboard operations should avoid parallel threads and that Linux clipboard contents can remain owned by the application that copied them.
- `arboard`'s image support is enabled by a default feature but is unnecessary for this text-only design.

## Locked Decisions

- Normal Session views should behave like a conventional terminal for mouse selection and clipboard interaction.
- TUI edit mode should replicate GNU nano editor behavior for comparable editing and selection interactions, except where this document explicitly locks a different ZT shortcut or behavior.
- Mouse-drag text selection and `Ctrl+Shift+C` copy are available in every TUI Session mode, not only in edit mode.
- In normal Session view, `Ctrl+Shift+V` pastes operating-system clipboard text into the one-line command bar.
- In TUI edit mode, `Ctrl+Shift+V` pastes operating-system clipboard text into editable Card text.
- Copy uses `Ctrl+Shift+C`.
- Paste uses `Ctrl+Shift+V`.
- Plain `Ctrl+C` keeps its existing meanings: exit from normal Session view and cancel from TUI edit mode.
- `Esc` also remains available to cancel TUI edit mode.
- Copy and paste use the operating-system clipboard, not a Session-local buffer.
- No alternative copy or paste binding is provided. If the terminal intercepts `Ctrl+Shift+C` or `Ctrl+Shift+V`, the user must change the terminal configuration so ZT receives the shortcut.
- In edit mode, typing or `Enter` replaces the selected editable text; `Backspace` and `Delete` remove it; and `Ctrl+Shift+V` replaces it with clipboard text.
- After replacement or deletion of selected editable text, the edit caret is at the start of the former selection.
- Outside edit mode, mouse dragging selects visible screen text exactly as rendered.
- Normal-view selection may include Card text, titles, Locations, help text, messages, and command-bar text.
- Copying a normal-view selection omits trailing blank terminal cells and inserts `\n` only at real output-line boundaries.
- Terminal-width soft wrapping does not add a newline to copied normal-view text; a soft-wrapped line copies as one logical output line.
- In normal Session view, `Ctrl+Shift+V` appends clipboard text to the end of the one-line command bar.
- Command-bar paste converts clipboard line breaks to spaces and never executes the command automatically; the user must still press `Enter`.
- A left click without mouse movement retains the existing Link-activation behavior.
- Any mouse movement while the left button is held starts text selection and suppresses Link activation for that gesture.
- In edit mode, copying uses the underlying selected Card text rather than rendered terminal cells.
- Edit-mode copy preserves exact Unicode characters, tabs, spaces, and logical `\n` line breaks.
- Dragging against an edit-viewport edge automatically scrolls vertically or horizontally and extends the selection beyond the visible area.
- Outside edit mode, selection cannot extend beyond the currently visible screen text.
- Selected text uses reverse-video highlighting in every TUI Session mode.
- Link styling displays normally whenever the Link is not part of the active selection.
- A successful copy keeps the selection highlighted.
- A new click or drag replaces the current selection.
- In normal Session view, command-bar editing or paste, command execution, and a view or mode change clear the selection.
- In edit mode, replacement or deletion of selected text clears the selection as part of the edit.
- When an arrow, `Home`, or `End` is pressed with an edit-mode selection active, the selection clears and the requested movement proceeds from the drag-release endpoint.
- Normal-view screen selection snaps both endpoints to whole displayed characters and never copies half of a wide character.
- Outside edit mode, tabs are copied as the spaces visibly rendered on screen.
- Copying leaves the selected text highlighted. This intentionally differs from GNU nano, which clears its mark after copying.
- Direct left-button dragging selects text without requiring `Shift`. This intentionally differs from GNU nano's terminal-native Shift-drag behavior.
- In edit mode, a single left click without dragging clears the active selection and places the edit caret at the clicked character boundary, matching nano's caret-placement behavior.
- `Ctrl+Shift+C` with no active selection is a silent no-op and never falls through to plain `Ctrl+C` behavior.
- Edit-mode paste normalizes `\r\n` and lone `\r` line endings to `\n`.
- Edit-mode paste otherwise preserves Unicode characters, tabs, blank lines, spaces, and trailing spaces exactly.
- Successful clipboard operations show no extra message; the persistent selection highlight or visibly inserted text is sufficient feedback.
- If an operating-system clipboard operation fails, the fixed status line shows the error and the selection, edit buffer, edit caret, and command bar remain unchanged.
- Selection starts only through direct mouse dragging. This feature does not add `Shift+Arrow`, nano mark shortcuts, or any other keyboard-selection mechanism.
- Double-click and triple-click have no special word-selection or line-selection behavior.
- Normal-view screen selection clears when terminal resizing or asynchronous redraw invalidates its rendered coordinates.
- Edit-mode selection survives terminal resizing because it is anchored to logical Card-text positions; its highlight and viewport are recomputed for the new terminal size.
- Edit-mode paste permits `\n` and tabs but rejects the entire clipboard payload if it contains any other control character.
- Command-bar paste converts newlines and tabs to spaces and rejects the entire clipboard payload if it contains any other control character.
- Invalid clipboard content is never pasted partially.
- With no edit-mode selection, valid clipboard text is inserted at the edit caret.
- `Tab` follows the same selected-text replacement rule as other inserted text.
- Right-click and middle-click do not copy or paste; copy and paste remain available only through `Ctrl+Shift+C` and `Ctrl+Shift+V`.
- Drag selection uses a normalized half-open character range, behaves identically when dragged forward or backward, includes every crossed character and intervening logical newline, and snaps beyond a rendered line to its logical end.
- Edit-mode edge auto-scroll advances one row or one display column for each drag event received at an edge.
- Edge auto-scroll has no background timer, acceleration, or separate scrolling subsystem.
- ZT uses `arboard` 3.6 as its single operating-system clipboard dependency, with default image features disabled and Wayland clipboard support enabled.
- One clipboard handle lives for the entire TUI Session and is accessed only from the existing event-loop thread.
- Clipboard access has no global singleton, worker thread, retry loop, or asynchronous layer.
- Normal Session view owns one small screen-selection model based on rendered terminal cells.
- `EditorModel` owns one logical Card-text selection range.
- Operating-system clipboard access remains outside both selection models so their behavior can be unit tested deterministically.
- This feature does not introduce a new TUI framework.
- Default automated tests do not read or modify the user's real operating-system clipboard.
- Clipboard controller and model tests use a small deterministic in-memory fake.
- Unit tests cover normal-view selection extraction, editor selection and replacement semantics, forward and backward ranges, Unicode and tab handling, newline normalization, invalid control characters, selection lifecycle, clipboard failures, and viewport behavior.
- PTY tests cover direct mouse dragging, reverse-video highlighting, Link click versus drag, Chinese text, and edit-mode edge scrolling without depending on the real operating-system clipboard.
- Manual acceptance uses Windows Terminal with the real operating-system clipboard and covers external copy/paste into both the one-line command bar and editable Card text.
- Acceptance requires `cargo test`, `cargo check`, and completion of the manual smoke checklist.
- Implementation changes are limited to `Cargo.toml`, `Cargo.lock`, `src/session.rs`, focused Session tests, and this design document.
- This feature does not refactor storage, Card parsing, or Session command routing and does not introduce a general screen or TUI framework.
- On Linux without a clipboard manager, copied data may cease to be available after the TUI Session exits. ZT accepts that platform-native clipboard lifetime and does not add a background clipboard keeper or daemon integration.
- The piped non-TTY Session remains unchanged and does not gain mouse selection or operating-system clipboard shortcuts.

## Baseline Precedence

- The intended precedence is: explicit decisions in this document, then GNU nano-like behavior inside TUI edit mode, then conventional terminal behavior in other TUI Session modes, then unchanged existing ZT behavior.
- Conflicts between an explicit decision and the nano or terminal baseline are resolved explicitly during this grilling session.

## Intentional Nano Deviations

- ZT uses `Ctrl+Shift+C` and `Ctrl+Shift+V` with the operating-system clipboard instead of nano's cutbuffer bindings.
- ZT keeps a selection highlighted after copying it.
- ZT starts selection with direct left-button dragging instead of requiring `Shift` or a separately toggled mark.

## Automated Verification

- Unit tests cover normal-view selection extraction, logical editor selection, forward and backward dragging, replacement and deletion, click-to-caret placement, Chinese characters, tabs, logical newlines, soft wraps, viewport offsets, resize behavior, invalid controls, and clipboard errors through an in-memory fake.
- PTY tests add named mouse press, drag, and release helpers and cover direct dragging, reverse-video highlighting, Link click versus drag, Chinese text, editor edge scrolling, and preservation of plain `Ctrl+C` behavior.
- Automated tests never depend on or modify the user's real operating-system clipboard.
- Acceptance requires `cargo test` and `cargo check`.

## Manual Windows Terminal Smoke Checklist

- Ensure Windows Terminal forwards `Ctrl+Shift+C` and `Ctrl+Shift+V` to ZT instead of intercepting them.
- Start the service and open a TUI Session with plain `zt`.
- In `ROOT` and Card views, drag forward and backward across English and Chinese text; verify reverse-video highlighting.
- Copy a normal-view selection with `Ctrl+Shift+C`, paste it into an external application, and verify visible text, real line boundaries, Chinese characters, omitted terminal padding, and no newline added by terminal soft wrapping.
- Click a rendered Link without dragging and verify navigation; drag across the same Link and verify selection without navigation.
- Copy multiline external text, then use `Ctrl+Shift+V` in normal Session view; verify it appends to the command bar, converts line breaks and tabs to spaces, and does not execute until `Enter` is pressed.
- Enter edit mode, single-click to place the edit caret, then drag forward and backward to select editable Card text.
- Verify `Ctrl+Shift+C` copies exact underlying edit text, including Chinese characters, tabs, spaces, and logical newlines, while leaving the selection highlighted.
- Verify typing, `Tab`, `Enter`, `Backspace`, `Delete`, and `Ctrl+Shift+V` replace or delete selected edit text and leave the caret at the start of the former selection.
- Paste multiline CRLF text into the editor and verify line endings normalize to `\n` while blank lines, tabs, spaces, trailing spaces, and Unicode remain intact.
- Drag against every edit-viewport edge and verify event-driven vertical and horizontal scrolling extends the logical selection.
- Resize the terminal and verify normal-view selection clears while edit-mode logical selection survives and redraws correctly.
- Press `Ctrl+Shift+C` with no selection and verify it is a silent no-op.
- Verify plain `Ctrl+C` still exits normal Session view and still cancels edit mode; verify `Esc` still cancels edit mode.
- Save, reopen the Card, and verify the final edited text exactly.
- Quit the Session and verify terminal raw mode, alternate-screen state, and mouse capture are restored normally.

## Implementation Authority

- This document is the locked implementation authority for Session mouse selection and operating-system clipboard behavior.
- Reopen the design only if implementation exposes a contradiction with the current terminal event stream or an operating-system clipboard limitation.
- No ADR is required because this change is localized, reversible, and does not establish a system-wide architectural commitment.
