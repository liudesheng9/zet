## Parent

#1

## What to build

Implement operational Session tracking and edit-lock behavior so daemon lifecycle, interactive Sessions, shell commands, and edit operations remain consistent.

## Acceptance criteria

- [ ] The daemon tracks open interactive Sessions.
- [ ] A Session registers when the interactive terminal view starts.
- [ ] A Session unregisters on normal exit.
- [ ] The daemon drops a Session if its IPC connection closes.
- [ ] `zt down` refuses to stop while Sessions are open and shows open Session count.
- [ ] There is no force-stop command in the initial design.
- [ ] If the daemon exits unexpectedly, the Session shows a one-line `service disconnected` message and exits without writing data.
- [ ] The daemon tracks one global edit lock.
- [ ] Only one edit operation can be active at a time, whether shell or Session.
- [ ] Commands needing the edit lock fail immediately when the edit lock is held.
- [ ] Edit-lock failures show a clear `edit in progress` message.
- [ ] Shell Card commands are rejected while any interactive Session is currently editing a Card.
- [ ] Read-only shell commands can run while a Card is being edited.
- [ ] Read-only shell commands include `zt status`, `zt stats`, `zt lsbk`, `zt version`, and `zt config show`.

## Blocked by

#6
