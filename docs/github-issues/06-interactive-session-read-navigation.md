## Parent

#1

## What to build

Implement the read/navigation side of the interactive terminal Session. The Session should start with plain `zt`, render ROOT/Topic/Card views, and support pointer navigation without write operations.

## Acceptance criteria

- [ ] Plain `zt` starts an interactive terminal Session only when the service is up.
- [ ] A new Session starts with its pointer on `ROOT`.
- [ ] `ROOT` view shows each Topic Location and title, with no description preview.
- [ ] Topic Card view shows Topic Location, title, description text, Reverse links, and direct successor link to `<topic>/1` when it exists.
- [ ] Regular Card view shows Location, title, Card text, Reverse links, direct successor link, and side successor links.
- [ ] Side successor links are ordered by Location label order: `a..z`, then `aa`, `ab`, and so on.
- [ ] Link macros render as highlighted selectable links, with mouse activation where the terminal supports it.
- [ ] Activating a valid rendered link moves the pointer to the target Card.
- [ ] Broken links cannot be jumped to from the interface.
- [ ] The one-line command bar accepts full `zt ...` commands and has no command history.
- [ ] `zt root` moves the pointer back to `ROOT`.
- [ ] `zt go <location>` moves the pointer to an existing non-broken Location.
- [ ] `zt go <location>` rejects missing and broken targets with one-line errors and leaves the pointer unchanged.
- [ ] `zt ls` lists Card Locations and titles under the current Topic.
- [ ] `zt q` and `Ctrl+C` exit the Session without changing Card data.

## Blocked by

#5
