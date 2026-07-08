# ZT Note System PRD

## Problem Statement

The user wants a local note system named `zt` that behaves like a CLI-first card archive. Existing note tools are either too UI-heavy, too file-oriented, or too loose about location, linking, backlinks, and safe structural edits. The user needs a Rust command-line tool that stores Cards in SQLite, keeps the service available through an explicit daemon lifecycle, and gives both an interactive terminal Session and script-friendly shell commands.

The core problem is not only storing text. The system must preserve a strict Location model, support direct and side successors, render links and reverse links, prevent malformed Cards from being saved, and make destructive operations such as delete and move explicit and auditable.

## Solution

Build `zt` as a Rust CLI application backed by a per-user daemon and a SQLite database under a configured `archive_root`. The daemon owns SQLite access while the service is up. Users start and stop it with `zt up` and `zt down`, inspect it with `zt status`, and configure storage with `zt config`.

Cards are the canonical note item. A Topic is a special Card at `<topic>/0`; regular Cards live under that Topic with successor Locations such as `1/1`, `1/2`, `1/2|c`, and `1/2|c|4|b|b`. Direct successors use numeric segments. Side successors use lowercase Excel-style letter labels. Each Card stores a single text representation with three sections separated by `<--->`: title, body text, and generated reverse links.

The primary user experience is an interactive terminal Session started by plain `zt`. It starts at `ROOT`, renders Topics and Cards, provides a one-line command bar, and uses a Session pointer for commands. Pointer-dependent card commands can also run from the shell with `--at <location>` while the service is up.

The implementation must keep the design simple but strict: no GUI, no web UI, no initial text search, no structural link table, no schema migrations during the development period, and no force-stop command.

## User Stories

1. As a user, I want to configure `archive_root`, so that I can choose where the SQLite archive lives.
2. As a user, I want `zt config show`, so that I can see the current configuration.
3. As a user, I want `zt config set archive_root <path>` to work while the service is down, so that I can change storage safely.
4. As a user, I want `zt config set archive_root <path>` rejected while the service is up, so that the active daemon cannot split across databases.
5. As a user, I want `zt up` to start the daemon, so that note operations can run.
6. As a user, I want `zt up` to create `archive_root` and initialize the database, so that first startup is simple.
7. As a user, I want `zt up` to fail if `archive_root` is not configured, so that notes are not stored in an unexpected location.
8. As a user, I want `zt up` to be a no-op if already running, so that repeated startup is harmless.
9. As a user, I want `zt down` to stop the daemon, so that I can close the service cleanly.
10. As a user, I want `zt down` to refuse while Sessions are open, so that active terminal views are not disconnected accidentally.
11. As a user, I want `zt status` to show daemon state, PID, archive root, SQLite path, card count, and open Session count, so that I can inspect service health.
12. As a user, I want `zt version` to print a simple version string, so that shell scripts can verify the installed tool.
13. As a user, I want shell command failures to use nonzero exit codes, so that `zt` is script-friendly.
14. As a user, I want plain `zt` to start an interactive Session only when the service is up, so that viewing and editing always use the daemon.
15. As a user, I want a new Session to start at `ROOT`, so that startup is predictable.
16. As a user, I want `ROOT` to show Topic locations and titles only, so that the first view stays compact.
17. As a user, I want a one-line command bar, so that I can run `zt` commands without leaving the Session.
18. As a user, I want command-bar commands to include the full `zt` prefix, so that session and shell syntax are consistent.
19. As a user, I want no command history in the command bar, so that the TUI stays simple.
20. As a user, I want `zt q` and `Ctrl+C` to exit the Session, so that I can leave predictably.
21. As a user, I want `zt root`, so that I can return to the Topic list.
22. As a user, I want `zt go <location>`, so that I can jump to a known Card.
23. As a user, I want `zt go` to reject missing or broken targets, so that the pointer never moves to nowhere.
24. As a user, I want `zt help` in the shell and Session, so that I can see available commands for the current context.
25. As a user, I want `zt stats` in the shell and Session, so that I can see total, Topic, and regular Card counts.
26. As a user, I want `zt ls` in the Session, so that I can list Card locations and titles under the current Topic.
27. As a user, I want to create a Topic with `zt t "Topic title"`, so that I can start a new root topic.
28. As a user, I want a Topic Card at `<topic>/0`, so that every Topic is addressable.
29. As a user, I want Topic root numbers to be monotonic and not reused, so that deleted Topics do not cause confusing old references.
30. As a user, I want Topic titles validated before creation, so that empty or multiline Topic titles are rejected early.
31. As a user, I want Topic creation to enter edit mode, so that I can immediately add a description.
32. As a user, I want Topic descriptions to reject link macros, so that Topics do not contain outbound links.
33. As a user, I want Topic Cards to receive reverse links, so that Cards can refer back to Topics.
34. As a user, I want `zt n` from a Topic to create `<topic>/1`, so that the first regular Card is predictable.
35. As a user, I want `zt n` to reject if the direct successor already exists, so that each Card has only one direct successor.
36. As a user, I want `zt n` from a regular Card ending in a number to increment that number, so that direct succession is clear.
37. As a user, I want `zt n` from a Card ending in a side label to append `|1`, so that direct succession after a side branch is clear.
38. As a user, I want `zt b` to create the next side successor, so that I can branch from a regular Card.
39. As a user, I want `zt b` rejected on Topic Cards, so that Topics only have the first regular Card as their direct successor.
40. As a user, I want side labels to continue `a..z, aa, ab...`, so that side successors are unbounded.
41. As a user, I want new regular Cards created with two `<--->` separators, so that the title, body, and reverse-link sections are present.
42. As a user, I want newly-created Cards to enter edit mode, so that creation and first content entry are one flow.
43. As a user, I want cancel-before-first-save to discard a newly-created Card, so that empty accidental Cards are not left behind.
44. As a user, I want `zt e` and shell `zt e --at <location>`, so that I can edit from the Session or shell.
45. As a user, I want shell edit to use `$EDITOR`, so that shell workflows fit my environment.
46. As a user, I want shell edit to fail clearly when `$EDITOR` is not set, so that failures are actionable.
47. As a user, I want TUI edit and shell edit to use the same validation and save pipeline, so that behavior is consistent.
48. As a user, I want malformed saves to stay in edit mode and show one error line, so that I can fix the Card without losing work.
49. As a user, I want the reverse-link section visible during editing, so that generated backlinks are inspectable.
50. As a user, I want reverse-link edits overwritten on save, so that generated text remains authoritative.
51. As a user, I want link macros to be `[[location]]` only, so that link parsing stays simple.
52. As a user, I want no link display text, so that the `|` character remains part of Location syntax.
53. As a user, I want save to reject newly introduced broken links, so that normal edits do not create dead references.
54. As a user, I want existing broken links to remain saveable, so that I can edit Cards after deletion without fixing every old reference immediately.
55. As a user, I want reverse links regenerated after save, delete, and move, so that backlinks stay current.
56. As a user, I want only one reverse-link line per source Card, so that repeated links from one Card do not spam backlinks.
57. As a user, I want reverse-link lines to include source Location and title, so that backlinks are useful.
58. As a user, I want generated reverse-link sections ignored during backlink computation, so that reverse links do not recursively create backlinks.
59. As a user, I want `zt lsbk`, so that I can find broken link macros.
60. As a user, I want `zt lsbk` to work in the Session and shell while service is up, so that broken links are easy to audit.
61. As a user, I want `zt del` to show a verification chart, so that I know exactly what will be deleted.
62. As a user, I want regular deletion to require typing `delete`, so that destructive regular Card deletion is intentional.
63. As a user, I want Topic deletion to require typing the Topic location, so that whole-topic deletion is harder to do by mistake.
64. As a user, I want deleting a Topic to remove the entire Topic, so that no orphaned Cards remain under that Topic.
65. As a user, I want deleting a direct successor not to renumber direct successor Locations, so that direct chains remain stable.
66. As a user, I want deleting a side successor to compact later side-successor subtrees, so that side-label gaps are filled.
67. As a user, I want delete compaction to rewrite affected link macros and regenerate reverse links, so that references remain consistent.
68. As a user, I want the pointer to land on the parent after side-successor deletion, so that I remain near the changed structure.
69. As a user, I want `zt mv --at <location> <new-location>` and Session `zt mv <new-location>`, so that I can move a Card subtree.
70. As a user, I want move rejected for Topic Cards, invalid Locations, self-subtree moves, skipped side labels, and destination conflicts, so that structural moves remain safe.
71. As a user, I want move verification charts and `move` confirmation, so that structural rewrites are explicit.
72. As a user, I want move to rewrite link macros and regenerate reverse links, so that existing references remain consistent.
73. As a user, I want shell `zt n --at` and `zt b --at` to print the new Location after successful save, so that scripts can capture it.
74. As a user, I want one global edit lock, so that concurrent edit writes do not conflict.
75. As a user, I want read-only commands to continue while editing, so that status and audit commands remain available.
76. As a user, I want commands needing the edit lock to fail immediately with `edit in progress`, so that blocked writes are clear.
77. As a user, I want save, delete, and move to run as SQLite transactions, so that partial structural updates cannot persist.
78. As a user, I want WAL mode, so that daemon-owned SQLite reads stay responsive.
79. As a user, I want no migrations during the development period, so that the early schema can be reset quickly.
80. As a user, I want corrupted SQLite startup to fail without modifying the database, so that damage is not compounded.
81. As a user, I want daemon logs under `archive_root`, so that operational failures can be inspected.

## Implementation Decisions

- `zt` is implemented in Rust as a CLI-only application.
- The application has a per-user background daemon started by `zt up` and stopped by `zt down`.
- The daemon owns live SQLite access while the service is up.
- CLI commands talk to the daemon through a per-user local IPC mechanism: Windows named pipe on Windows and Unix domain socket on Unix-like systems.
- Service-down command availability is limited to `zt up`, `zt status`, `zt version`, `zt config show`, and `zt config set archive_root <path>`.
- Configuration is stored in a per-user OS config file. The only current key is `archive_root`.
- The SQLite file is `<archive_root>/zt.sqlite3`.
- The daemon writes minimal append-only logs to `<archive_root>/zt.log`.
- `zt up` creates `archive_root` recursively, initializes a missing database, enables WAL mode, and fails without modifying the database if corruption is detected.
- SQLite stores a single `cards` table with `location` as primary key, plus `is_topic` and `text`.
- Link relationships are never stored as structural SQLite data.
- Every Card text uses three sections separated by exact `<--->` lines: title, body or Topic description, and generated reverse links.
- Topic Cards use `<topic>/0`, are marked with `is_topic`, have descriptions that reject outbound link macros, and can receive reverse links.
- Regular Card Locations follow successor notation and must match the locked grammar.
- Topic IDs and numeric successor segments reject leading zeroes, except topic ID `0`.
- Topic root IDs are monotonic and not reused after deletion.
- Direct successors use numeric segments. A Card has at most one direct successor.
- Side successors use lowercase Excel-style labels. A Card may have many side successors.
- The public terms are `direct successor` and `side successor`.
- `zt n` and `zt b` create Cards, enter edit mode, and discard unsaved first-save cancellations.
- Pointer-dependent card commands can run in the Session command bar using the Session pointer or from the shell with `--at <location>`.
- Shell card commands do not use an implicit pointer.
- Shell editing uses `$EDITOR`, temporary `.zt.md` files, and deletes temporary files after save or cancel.
- Session editing uses the TUI editor with `Ctrl+S` to save and `Esc` to cancel.
- TUI editing and shell editing share one validation and save pipeline.
- The daemon tracks open Sessions and one global edit lock.
- `zt down` refuses while Sessions are open. There is no force-stop command in the initial design.
- Read-only commands can run during edits; commands needing the edit lock fail immediately with `edit in progress`.
- Save rejects malformed Cards, invalid link macros, and newly introduced broken links.
- Existing broken links may remain across saves until manually fixed.
- Reverse-link sections are generated text stored inside `text`, regenerated after every successful save, delete, or move.
- Reverse-link computation ignores links inside generated reverse-link sections.
- Deletion removes the target Card plus successors, with stronger confirmation for Topic deletion.
- Deleting side successors compacts later side-successor subtrees and rewrites affected link macros before broken-link reporting.
- Moving Cards moves the whole subtree, validates successor rules, rejects unsafe destinations, rewrites link macros, and regenerates reverse links.
- The initial design excludes text search, display-text links, GUI, web UI, command history, schema migrations, and force stop.

## Testing Decisions

- Favor behavior tests at the CLI/daemon boundary over implementation-detail tests.
- Use isolated temporary archive roots and SQLite databases for integration tests.
- Cover service lifecycle from shell commands: config, up, repeated up, down, repeated down, status, version, and corrupted database startup.
- Cover command availability while service is down and while service is up.
- Cover the Location parser and successor generator with focused unit tests.
- Cover Card text parsing and validation with table-driven tests.
- Cover Topic validation separately from regular Card validation.
- Cover shell `$EDITOR` flows with controlled test editors that save, cancel, or produce malformed content.
- Cover TUI command-bar behavior through the highest available terminal interaction seam.
- Cover global edit lock behavior with concurrent shell/session edit attempts.
- Cover reverse-link regeneration after save, delete, and move.
- Cover broken-link behavior, including newly introduced broken links versus pre-existing broken links.
- Cover delete confirmation charts, Topic deletion, direct deletion, side-successor compaction, link rewriting, and pointer movement.
- Cover move verification charts, destination validation, subtree moves, link rewriting, and pointer movement.
- Cover `zt lsbk`, `zt stats`, `zt ls`, `zt help`, `zt go`, and `zt root` as user-visible command behavior.
- Treat nonzero shell exit codes and user-facing error text as part of the external contract.

## Out of Scope

- GUI or web interface.
- Text search.
- Command history in the Session command bar.
- Link display text such as `[[location|label]]`.
- Structural SQLite link tables.
- Schema migrations during the development period.
- Force-stopping the daemon while Sessions are open.
- Moving Topic Cards.
- Manual Location selection during normal card creation outside the defined command behavior.
- Reusing Topic root numbers after deletion.

## Further Notes

- This PRD is based on the locked ZT note system design.
- The design is intentionally KISS: strict core behavior first, convenience features later.
- The implementation issues should preserve the vocabulary in `CONTEXT.md`: Card, Topic, Location, Pointer, Session, Link, Broken link, Reverse link, Direct successor, and Side successor.
