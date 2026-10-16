## Parent

#1

## What to build

Implement shell-driven Topic/Card creation and editing. Pointer-dependent Card commands must be runnable as shell commands with `--at <location>`, use the daemon, and share the same validation/save behavior as later Session editing.

## Acceptance criteria

- [ ] Shell `zt t "Topic title"` works while the service is up and does not require `--at`.
- [ ] Shell `zt t "Topic title"` rejects empty or multiline Topic titles before opening edit mode.
- [ ] Shell `zt t "Topic title"` creates the Topic Card at `<topic>/0`, opens `$EDITOR`, and prints the new Topic Location only after successful save.
- [ ] Shell `zt n --at <location>` creates the direct successor of the Card at `<location>` and prints the new Location after successful save.
- [ ] Shell `zt b --at <location>` creates the next side successor of the Card at `<location>` and prints the new Location after successful save.
- [ ] Shell `zt e --at <location>` opens the Card at `<location>` in `$EDITOR`.
- [ ] Shell edit fails clearly when `$EDITOR` is not set.
- [ ] Shell edit temporary files use the `.zt.md` extension.
- [ ] Shell edit temporary files are deleted after successful save or cancel.
- [ ] Canceling edit before a newly-created Card has ever been saved discards the newly-created Card.
- [ ] Shell Card commands use no implicit pointer.
- [ ] Shell Card commands return nonzero exit codes on validation, service, edit-lock, or save failures.

## Blocked by

#3
