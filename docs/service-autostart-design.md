# Service autostart design

The ZT service follows the machine: it comes up when the user signs in and goes down when the
machine ends the user's session.

## Up at sign-in

- `zt autostart on` writes the value `zt` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
  The Run key needs no admin rights (a logon task in Task Scheduler does), the user can see and
  toggle it in Task Manager's Startup apps, and the archive's config lives in the user's profile,
  so sign-in is the right moment.
- The value runs `zt-boot.exe`, a second binary of the crate built for the Windows subsystem, so
  signing in opens no console (not even a Windows Terminal tab). It runs `zt __boot` beside it
  with `CREATE_NO_WINDOW`. When `ZT_CONFIG_DIR` was set at `zt autostart on`, the command carries
  `--config-dir` so sign-in uses the same archive.
- `zt __boot` is `zt up` that records its outcome in `<config dir>\autostart.log` (and a
  `login start:` line in `zt.log`), because nobody sees its output. `zt autostart` shows the
  last outcome.
- `zt up` starts `zt __daemon <root>` directly with `CREATE_NO_WINDOW` and null stdio. It needs
  no shell on PATH and passes archive paths as one argument, including spaces and apostrophes.
  It waits up to 15 seconds for the service record while the machine is still signing in.

## Down with the machine

- The daemon links user32 (through the clipboard crate), so Windows tells it about log-off and
  shutdown with window messages, not console control events. The daemon owns a hidden top-level
  window (class `ztDaemon`, title `zt daemon <pid>`) that answers `WM_QUERYENDSESSION` with yes
  and, on `WM_ENDSESSION`, removes `zt.pid` and logs `service stopped: <reason>` before the
  system ends the process. It does not wait for open Sessions; they end with the machine too.
- A daemon can still end without warning (a power cut, a killed process). It holds an exclusive
  lock on `zt.daemon.lock` for its whole life, which the system releases however it ends. A
  `zt.pid` whose lock is free is a stale record: any check of the service clears it, logs
  `service record of daemon <pid> cleared`, starts the service again, and logs
  ``service restarted: daemon <pid> ended without `zt down` `` (or `service did not restart:
  <error>`). So the service stays down only after `zt down` or when the machine ends the
  session; both remove `zt.pid`, which leaves no stale record. `zt up`, `zt down` and
  `zt config set` clear a stale record without the restart. A record from a zt 0.7 daemon (no
  lock file) is checked by its pid when `zt up` runs.
- Several processes can find the same stale record (agents call zt in parallel). The first one
  creates `zt.restart` with create-new and restarts the service; the others, and any process
  that finds no `zt.pid` while `zt.restart` exists, wait for the new daemon. The restarting
  process checks the record again after it claims `zt.restart`, so it never clears a daemon that
  another process has just started. A `zt.restart` older than 30 seconds is from a process that
  died during a restart and is ignored. `zt up` waits for a restart in progress, since a second
  daemon would only wait for the daemon lock.
- Why restart: on 2026-10-13 and 2026-10-15 the daemon ended with no log line and no crash
  report while the Claude desktop app crashed and restarted, and the service stayed down until a
  person ran `zt up`.

## Verification

`tests/autostart.rs` registers under a scratch key (`ZT_AUTOSTART_KEY`), signs in by running the
registered command line the way Windows does, and ends the session by sending the daemon's
window the messages Windows sends at shutdown and sign-out. It covers sign-in, a failed sign-in,
shutdown with an open Session, sign-out, a cancelled shutdown, a full restart, a daemon killed
like a power cut (the next command restarts it, and `zt down` still keeps it down), four
commands that find the same killed daemon at once (one restart), and a record left by an older
daemon.
