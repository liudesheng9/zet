## Parent

#1

## What to build

Implement write operations from the Session command bar and TUI edit mode. Session writes should use the Session pointer and share the same Card validation/save pipeline as shell editing.

## Acceptance criteria

- [ ] Session command bar supports `zt t "Topic title"`, `zt n`, `zt b`, `zt e`, `zt del`, and `zt mv <new-location>`.
- [ ] Commands entered through the Session command bar use the Session pointer where applicable.
- [ ] Session `zt t "Topic title"` creates a Topic Card, moves the pointer to `<topic>/0`, and enters edit mode.
- [ ] Session `zt n` creates the direct successor of the current Card, rejects when a direct successor already exists, and enters edit mode.
- [ ] Session `zt b` creates the next side successor of the current regular Card, rejects on Topic Cards, and enters edit mode.
- [ ] TUI edit mode shows the full three-section Card text, including generated Reverse links.
- [ ] TUI edit mode uses `Ctrl+S` to save and `Esc` to cancel.
- [ ] TUI edit mode and shell `$EDITOR` use the same Card text validation and save pipeline.
- [ ] User edits to the Reverse link section are overwritten by generated Reverse link text on save.
- [ ] If the Reverse link section is damaged during editing, save repairs it from stored Cards.
- [ ] Malformed saves keep the user in edit mode, show a one-line error, and do not write changes.
- [ ] Canceling before first successful save discards the newly-created Card.

## Blocked by

#6
