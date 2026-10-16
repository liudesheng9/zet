## Parent

#1

## What to build

Build the first runnable `zt` vertical slice: a Rust CLI that can configure an archive root, start and stop a per-user daemon, initialize SQLite storage, report status/version, and enforce service-up/service-down command availability. This slice should create the foundation every later Card operation will use.

## Acceptance criteria

- [ ] `zt config show` lists config items and currently includes `archive_root`.
- [ ] `zt config set archive_root <path>` works while the service is down and is rejected while the service is up.
- [ ] `zt up` fails clearly when `archive_root` is unset.
- [ ] `zt up` creates a missing `archive_root` recursively.
- [ ] `zt up` initializes `<archive_root>/zt.sqlite3` with a `cards` table using `location` as the primary key plus `is_topic` and `text`.
- [ ] `zt up` enables SQLite WAL mode.
- [ ] `zt up` is a no-op success when the daemon is already running and reports the PID.
- [ ] `zt down` stops the daemon and is a no-op success when already stopped.
- [ ] While the service is down, only `zt up`, `zt status`, `zt version`, `zt config show`, and `zt config set archive_root <path>` are available.
- [ ] `zt status` reports service state, daemon PID when up, archive root, SQLite path, card count when reachable, and open Session count.
- [ ] `zt version` prints only the CLI version in the form `zt 0.1.0`.
- [ ] Corrupted SQLite startup fails clearly and does not modify the database.
- [ ] The daemon writes minimal append-only logs to `<archive_root>/zt.log`.
- [ ] Shell command failures return nonzero exit codes.

## Blocked by

None - can start immediately
