# Session Interactive Tests and Edit Caret Issue Plan

Parent PRD: [#12](https://github.com/liudesheng9/zet/issues/12)

Source design: [session-interactive-tests-and-edit-caret-design.md](session-interactive-tests-and-edit-caret-design.md)

This breakdown is published to GitHub with the `enhancement` and `ready-for-agent` labels. The closed issue archive files under `doc/github-issues/` are intentionally unchanged.

## Published Issues

- [#12 [PRD] Session PTY coverage and edit caret navigation](https://github.com/liudesheng9/zet/issues/12)
- [#13 Establish PTY Session test seam and Session module boundary](https://github.com/liudesheng9/zet/issues/13)
- [#14 Implement model-backed TUI edit caret navigation](https://github.com/liudesheng9/zet/issues/14)
- [#15 Wire nano-like TUI edit mode end to end](https://github.com/liudesheng9/zet/issues/15)
- [#16 Add PTY coverage for Session navigation and links](https://github.com/liudesheng9/zet/issues/16)
- [#17 Add PTY coverage for Session writes and edit-lock safety](https://github.com/liudesheng9/zet/issues/17)

## Vertical Slices

1. Establish PTY Session test seam and Session module boundary
   - Type: AFK
   - Blocked by: None
   - Covers the cross-platform PTY spike, reusable PTY helpers, the one-entrypoint Session module boundary, and preservation of the existing line-session fallback.

2. Implement model-backed TUI edit caret navigation
   - Type: AFK
   - Blocked by: #13
   - Covers the terminal-independent editor model, intent-named actions, Unicode-character line/column behavior, desired vertical column, and exhaustive model tests.

3. Wire nano-like TUI edit mode end to end
   - Type: AFK
   - Blocked by: #14
   - Covers the real TUI edit mode behavior: edit-caret movement, fixed header/status/footer rows, viewport scrolling, save/cancel/retry behavior, and Chinese text persistence.

4. Add PTY coverage for Session navigation and links
   - Type: AFK
   - Blocked by: #13
   - Covers real PTY tests for Session read/navigation commands, rendered link mouse activation, service-disconnect behavior, and fallback line-session regression preservation.

5. Add PTY coverage for Session writes and edit-lock safety
   - Type: AFK
   - Blocked by: #15 and #16
   - Covers PTY tests for write commands, edit save/cancel/retry, delete, move, user-visible edit-lock behavior, SQLite persistence assertions, Chinese text workflows, and the manual TUI smoke checklist.

## Acceptance Gate

- `cargo test`
- Manual local TUI smoke run using the checklist in the locked design document
