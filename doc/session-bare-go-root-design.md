# Session Bare `go` to `ROOT` Design

Status: design locked and implemented.

## Source Authority

- `CONTEXT.md` defines `Session`, `Pointer`, `ROOT`, `Location`, and `Citation key`.
- `doc/zt-note-system-design.md` is the current main product contract.
- `doc/session-bare-subcommands-design.md` defines bare Session command syntax.
- `doc/literature-card-design.md` defines a navigation `target` as either a Location or Citation key.
- `src/session.rs` owns both TUI and piped Session navigation and Session help.
- `src/main.rs` owns shell command parsing and is outside this feature's implementation boundary.

## Pre-implementation Code Facts

- Every new TUI or piped Session initializes its Pointer at `ROOT`.
- Session `root` moves the Pointer to `ROOT`.
- Session `go <target>` moves the Pointer to an existing Card addressed by a Location or Citation key.
- A missing `go <target>` target produces a one-line error and leaves the Pointer unchanged.
- Bare Session `go` fell through to `unknown session command: go`.
- Both Session command handlers implemented `root` and `go <target>` separately.
- Session help showed `root | go <target>`.

## Locked Decisions

- This is a Session-only feature.
- Do not add shell `zt go`.
- Do not add a shell `--at` form for `go`.
- Do not add an initial-Location or initial-target option to plain shell `zt`.
- In both the TUI Session and piped Session, bare `go` moves that Session's Pointer to `ROOT`.
- Session `go <target>` retains its existing Card-navigation behavior.
- Session `root` remains supported as a backward-compatible alias for moving the Pointer to `ROOT`.
- Session help presents the navigation forms as `go [<target>] | root`.
- Bare Session `go` behaves exactly like `root`: it moves the Pointer to `ROOT`, clears the TUI status message, and is an idempotent no-op when the Pointer is already at `ROOT`.
- In a piped Session, bare `go` moves the Pointer to `ROOT` and the normal Session loop redraws the `ROOT` view.
- Session `go` with more than one target-like argument fails with the one-line usage error `usage: go [<target>]`.
- The canonical navigation argument is `target`, not `direction id` or `ID`.

## Guardrails

- Keep the feature narrow: do not change shell parsing, Session startup, Card storage, Link resolution, editing, creation, deletion, moving, or service lifecycle behavior.
- Reuse the existing Pointer-to-`ROOT` behavior rather than introducing a second root-navigation path.
- Cover both the TUI Session and piped Session paths because both are supported Session implementations.
- Do not revise closed issue archive files under `doc/github-issues/`.

## Acceptance Contract

- TUI Session tests prove that `go` from a Card returns to `ROOT` and clears the current status message.
- TUI Session tests prove that `go` while already at `ROOT` succeeds without changing data or leaving an error.
- Piped Session tests prove the same Pointer transitions for bare `go`.
- Existing tests continue to prove navigation through `go <Location>` and `go <Citation-key>`.
- Session help tests expect `go [<target>] | root`.
- Both Session paths reject `go <target> <extra>` with `usage: go [<target>]`.
- Shell help must not list `zt go`, and shell `zt go` remains an unknown command.
- Acceptance requires `cargo fmt -- --check`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.

## Open Decisions

None currently identified.
