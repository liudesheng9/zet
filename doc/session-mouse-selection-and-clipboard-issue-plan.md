# Session Mouse Selection and Operating-System Clipboard Issue Plan

Parent PRD: [#35](https://github.com/liudesheng9/zet/issues/35)

Source design: [session-mouse-selection-and-clipboard-design.md](session-mouse-selection-and-clipboard-design.md)

This four-slice tracer-bullet breakdown is approved for publication. Every slice is AFK-ready, includes its own automated verification, and uses the `enhancement` and `ready-for-agent` labels.

## Published Issues

- [#35 [PRD] Session mouse selection and operating-system clipboard](https://github.com/liudesheng9/zet/issues/35)
- [#36 Select and copy rendered Session text in every TUI view](https://github.com/liudesheng9/zet/issues/36)
- [#37 Paste operating-system clipboard text into the Session command bar](https://github.com/liudesheng9/zet/issues/37)
- [#38 Select and copy logical Card text in TUI edit mode](https://github.com/liudesheng9/zet/issues/38)
- [#39 Paste and replace logical Card text in TUI edit mode](https://github.com/liudesheng9/zet/issues/39)

## Vertical Slices

1. **Select and copy rendered Session text in every TUI view**
   - Type: AFK
   - Blocked by: None
   - User stories: 1-18, 39-48, 51
   - Delivers one complete non-editor drag/highlight/copy path through terminal events, rendering, operating-system clipboard access, errors, and tests.

2. **Paste operating-system clipboard text into the Session command bar**
   - Type: AFK
   - Blocked by: #36
   - User stories: 14-16, 19-23, 39-47, 51
   - Delivers one complete normal-view paste path through clipboard access, sanitization, command-bar state, redraw, errors, and tests.

3. **Select and copy logical Card text in TUI edit mode**
   - Type: AFK
   - Blocked by: #36
   - User stories: 11-13, 24-30, 39-49, 51
   - Delivers one complete editor click/drag/highlight/copy path through logical Card positions, viewport behavior, operating-system clipboard access, and tests.

4. **Paste and replace logical Card text in TUI edit mode**
   - Type: AFK
   - Blocked by: #37 and #38
   - User stories: 31-51
   - Delivers editor paste/replacement end to end and closes the cross-mode verification gate with the real Windows Terminal smoke checklist.

No separate horizontal test ticket is created. Each slice owns its model/controller/PTY evidence, and Slice 4 owns the final cross-mode manual acceptance gate.

## Published Issue Specifications

### Slice 1

<!-- ISSUE_1_BODY_START -->
## Parent

- #35

## What to build

Add direct left-button drag selection and `Ctrl+Shift+C` copy across every non-editor TUI Session view, including `ROOT`, Card, help/message output, and modal confirmation or selection screens.

The completed slice must retain rendered terminal cells and real-versus-soft line boundaries closely enough to highlight and serialize visible screen text correctly. It must copy whole Chinese characters, expand displayed tabs, omit terminal padding, preserve real output newlines, suppress soft-wrap newlines, and keep selection highlighted after copy.

Use one synchronous text-only operating-system clipboard adapter for the TUI Session lifetime. A click without movement still activates a rendered Link; any drag selects without navigation. Plain `Ctrl+C`, terminal teardown, and the piped line-session fallback remain unchanged.

## Acceptance criteria

- [ ] Direct forward and backward left-button dragging selects visible text in every non-editor TUI Session view, including at least one modal confirmation or selection view.
- [ ] Selection is rendered with reverse video and snaps to whole display characters, including Chinese wide characters.
- [ ] Copied text omits trailing terminal padding, expands visible tabs to spaces, inserts newlines only at real output-line boundaries, and does not add newlines at terminal soft wraps.
- [ ] `Ctrl+Shift+C` copies a nonempty selection to the operating-system clipboard and leaves it highlighted; with no selection it is a silent no-op.
- [ ] Successful copy shows no extra success message; the persistent selection highlight is the feedback.
- [ ] A new gesture replaces selection, while command input/execution, view changes, invalidating redraws, and terminal resize clear screen-based selection.
- [ ] A no-movement click activates an existing Link, while dragging over the Link never changes the Session Pointer.
- [ ] Copy/paste remains keyboard-only: right/middle click, double/triple click, keyboard selection, and alternate clipboard shortcuts are not added.
- [ ] If the terminal intercepts `Ctrl+Shift+C`, the user must reconfigure the terminal; ZT adds no fallback binding.
- [ ] Clipboard failures use the existing status/message area and preserve selection and Session state.
- [ ] Clipboard access uses one event-loop-owned text-only `arboard` handle with image features disabled and Wayland support enabled; no worker, retry loop, async layer, keeper, or daemon integration is added.
- [ ] Deterministic model/controller tests use an in-memory clipboard fake and cover selection extraction, Unicode cells, tabs, padding, line boundaries, lifecycle, and failure.
- [ ] PTY helpers gain named mouse press/drag/release and `Ctrl+Shift+C` sequences; PTY workflows prove highlighting, Link click-versus-drag, a representative modal view, Chinese text, and preserved plain `Ctrl+C` behavior without touching the user's real clipboard.
- [ ] Existing line-session behavior and terminal raw-mode/alternate-screen/mouse-capture restoration remain green under the full test suite.

## Blocked by

None - can start immediately.
<!-- ISSUE_1_BODY_END -->

### Slice 2

<!-- ISSUE_2_BODY_START -->
## Parent

- #35

## What to build

Add `Ctrl+Shift+V` paste from the operating-system clipboard into the normal Session's one-line command bar. Paste appends at the end, converts newlines and tabs to spaces, rejects any other control character atomically, clears active screen selection, and never executes until the user presses `Enter`.

Paste is limited to the command bar in normal Session view. It does not add command-bar caret movement or selection and does not paste into modal confirmation, verification, or edit-part selection inputs.

## Acceptance criteria

- [ ] `Ctrl+Shift+V` appends valid clipboard text to the end of the normal Session command bar.
- [ ] CRLF, lone CR, LF, and tab characters become ordinary spaces so the command bar remains one line.
- [ ] Paste never submits or executes the command; `Enter` remains required.
- [ ] Any control character other than normalized newline/tab rejects the complete paste with no partial command-bar change.
- [ ] Clipboard unavailability or conversion failure shows a status error and preserves command text and selection.
- [ ] Successful paste clears normal-view screen selection and otherwise produces no extra success message.
- [ ] Paste does not add command-bar caret movement, command-bar selection, modal-input paste, right/middle-click paste, or fallback shortcuts.
- [ ] Plain `Ctrl+C` continues to exit normal Session view, and terminals that intercept `Ctrl+Shift+V` require user configuration rather than a ZT fallback.
- [ ] Controller/model tests use the in-memory clipboard fake for append, normalization, rejection, failure atomicity, and selection lifecycle.
- [ ] PTY coverage proves key routing, visible appended text, newline/tab flattening, lack of auto-execution, and unchanged existing command execution after `Enter` without using the real OS clipboard.

## Blocked by

- #36
<!-- ISSUE_2_BODY_END -->

### Slice 3

<!-- ISSUE_3_BODY_START -->
## Parent

- #35

## What to build

Add nano-like mouse placement, direct drag selection, reverse-video highlighting, and `Ctrl+Shift+C` copy for logical Card text in TUI edit mode.

A single click places the Edit caret. A drag creates a normalized half-open logical range that preserves exact Unicode, tabs, spaces, and LF boundaries. Dragging at viewport edges scrolls one row or display column per drag event. Logical selection survives terminal resize, remains highlighted after copy, and clears when caret movement continues from the drag-release endpoint.

## Acceptance criteria

- [ ] A single left click without dragging clears selection and places the Edit caret at the clicked logical character boundary.
- [ ] Direct forward and backward dragging selects the same normalized half-open Card-text range without requiring `Shift` or nano mark keys.
- [ ] Selection highlighting uses reverse video and maps Chinese characters, tabs, row offsets, and display-column offsets correctly.
- [ ] `Ctrl+Shift+C` copies exact underlying selected Card text, including Unicode, tabs, spaces, and logical LF boundaries, and leaves the selection highlighted.
- [ ] `Ctrl+Shift+C` with no selection is a silent no-op and never triggers plain `Ctrl+C` cancellation.
- [ ] Edge dragging advances vertical or horizontal viewport selection by one row/display column per drag event with no timer or acceleration.
- [ ] Terminal resize preserves the logical selection and recomputes the viewport/highlight.
- [ ] Arrow, `Home`, and `End` clear selection and then move from the drag-release endpoint using existing editor movement rules.
- [ ] Clipboard failure shows the fixed editor status error and preserves buffer, selection, Edit caret, and viewport.
- [ ] Unit tests cover click mapping, forward/backward ranges, multiline extraction, Chinese/tabs, viewport edges, resize, movement lifecycle, and errors using the clipboard fake.
- [ ] PTY tests cover click-to-caret, direct dragging, reverse-video highlighting, Chinese text, both edge-scroll axes, resize redraw, copied-selection persistence, and preserved plain `Ctrl+C` edit cancellation without touching the real OS clipboard.
- [ ] Existing nano-like editor movement, save, cancel, validation retry, and Card persistence behavior remain green.

## Blocked by

- #36
<!-- ISSUE_3_BODY_END -->

### Slice 4

<!-- ISSUE_4_BODY_START -->
## Parent

- #35

## What to build

Add `Ctrl+Shift+V` paste and standard selection-aware editing to TUI edit mode. Valid clipboard text inserts at the Edit caret when no selection exists. With a selection, typing, `Tab`, `Enter`, and paste replace it; `Backspace` and `Delete` remove it. Every replacement or deletion leaves the Edit caret at the former selection start.

Normalize CRLF and lone CR to LF, preserve Unicode, tabs, blank lines, spaces, and trailing spaces, and reject any other control character atomically. Complete the feature by running the full automated gate and the locked real Windows Terminal clipboard checklist across normal Session and editor modes.

## Acceptance criteria

- [ ] `Ctrl+Shift+V` inserts valid clipboard text at the Edit caret when no selection exists.
- [ ] Typing, `Tab`, `Enter`, and `Ctrl+Shift+V` replace selected logical Card text and leave the Edit caret at the selection start.
- [ ] `Backspace` and `Delete` remove selected logical Card text and leave the Edit caret at the selection start.
- [ ] Paste normalizes CRLF and lone CR to LF while preserving Unicode, tabs, blank lines, spaces, and trailing spaces exactly.
- [ ] Any other control character rejects the complete paste without partial buffer, selection, caret, or viewport mutation.
- [ ] Clipboard failure shows the fixed editor status error and preserves buffer, selection, Edit caret, and command-bar state.
- [ ] Successful paste or replacement clears selection and shows no extra success message beyond visibly changed text.
- [ ] Save/reopen proves exact Card-text persistence after multiline Chinese, tab, blank-line, and trailing-space paste/replacement.
- [ ] Unit/model tests cover every replacement/deletion action, insertion without selection, newline normalization, exact preservation, invalid controls, and clipboard failures using the in-memory fake.
- [ ] PTY coverage proves editor paste routing and visible selection-aware edits while keeping automated tests independent of the real OS clipboard.
- [ ] Plain `Ctrl+C` retains its normal-view exit and edit-cancel behavior, and `Esc` retains edit cancellation.
- [ ] `cargo check` and the complete `cargo test` suite pass with existing Session, Link, Card validation, line-session, and terminal-teardown regressions intact.
- [ ] The manual Windows Terminal checklist from the locked design passes with real external copy/paste in normal and edit modes, Chinese and multiline text, Link click-versus-drag, edge scrolling, resize behavior, plain `Ctrl+C`, save/reopen fidelity, and terminal restoration.
- [ ] Implementation remains limited to the Session/clipboard dependency and focused Session tests; no storage, Card parser, command-routing, TUI-framework, clipboard-history, cut, keyboard-selection, or unrelated refactor is introduced.

## Blocked by

- #37
- #38
<!-- ISSUE_4_BODY_END -->

## Coverage Map

| Locked design area | PRD stories | Owning slice |
| --- | --- | --- |
| Every non-editor TUI mode can drag, highlight, and copy visible rendered text | 1-18 | Slice 1 |
| Operating-system clipboard adapter, shortcut contract, failures, and platform lifetime | 11-13, 23, 39-47 | Slice 1, then reused by Slices 2-4 |
| One-line command-bar paste, sanitization, and no auto-execution | 19-23 | Slice 2 |
| Editor click, logical drag selection, exact copy, movement, scrolling, and resize | 24-30 | Slice 3 |
| Editor insertion, replacement, deletion, normalization, preservation, and persistence | 31-38 | Slice 4 |
| Shortcut preservation, KISS exclusions, line-session regression, deterministic tests, PTY coverage, Windows smoke, and terminal restoration | 39-51 | Every slice; final gate in Slice 4 |

## Publication Audit

- Audited on 2026-07-14 against the live GitHub repository with `gh`.
- PRD #35 and implementation issues #36-#39 are open and carry both `enhancement` and `ready-for-agent`.
- Each remote title and body exactly matches the corresponding local PRD or published issue specification.
- The PRD contains 51 numbered user stories.
- The semantic coverage audit checked 26 requirement families spanning every TUI mode, both clipboard shortcuts, shortcut preservation, screen serialization, Unicode/tabs, Link gestures, selection lifecycle, command paste, editor selection and replacement, normalization, atomic failures, KISS exclusions, platform ownership, automated seams, manual acceptance, and terminal restoration.
- No pending publication text or unresolved dependency placeholder remains.
