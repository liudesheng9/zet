# Bare Session `go` Returns to `ROOT` PRD

Status: implemented and verified.

## Problem Statement

Before this change, a ZT Session had two navigation commands with unnecessarily different shapes. `go <target>` moved the Session Pointer to a known Card, while `root` returned the Pointer to `ROOT`. Entering bare `go` produced `unknown session command: go`, even though returning to `ROOT` is the natural no-target form of the same navigation action.

This inconsistency existed in both supported Session paths: the raw terminal TUI and the piped line-session fallback. It also made Session help less expressive because it presented `root` and `go <target>` as unrelated commands. The change remains strictly inside a Session; a shell process has no Session Pointer, and the user explicitly does not want a shell `zt go`, a `go --at` form, or a target-selection option for plain `zt`.

## Solution

Make the target optional for the bare Session `go` command. `go <target>` keeps navigating to an existing Card addressed by a Location or Citation key. Bare `go` performs the same Pointer-to-`ROOT` transition as the retained `root` alias.

In the TUI, bare `go` also clears the current status message, exactly like `root`. In a piped Session, the normal Session loop redraws the `ROOT` view after the transition. Calling bare `go` while already at `ROOT` succeeds as an idempotent no-op. Session help advertises `go [<target>] | root`, and more than one target-like argument fails with `usage: go [<target>]` in both Session paths.

Keep the implementation narrow. Do not change shell parsing, Session startup, target resolution, Card data, Links, editing, creation, deletion, moving, or service lifecycle behavior.

## User Stories

1. As a TUI Session user viewing a Card, I want to enter `go` without a target, so that I can return the Pointer to `ROOT`.
2. As a piped Session user viewing a Card, I want bare `go` to return the Pointer to `ROOT`, so that both supported Session paths use the same navigation contract.
3. As a Session user, I want bare `go` to mean the same thing as `root`, so that root navigation has one consistent behavior.
4. As a Session user already at `ROOT`, I want bare `go` to succeed as an idempotent no-op, so that I do not need to know my current Pointer before using it.
5. As a TUI Session user, I want bare `go` to clear the current status message, so that the resulting `ROOT` view is not accompanied by stale output.
6. As a piped Session user, I want the normal Session loop to redraw `ROOT` after bare `go`, so that the transition is visible in line-oriented output.
7. As an existing user, I want `root` to remain supported, so that established Session habits and scripts do not break.
8. As a Session user, I want `go <Location>` to retain its current behavior, so that Topic and Regular Card navigation does not regress.
9. As a Session user, I want `go <Citation-key>` to retain its current behavior, so that Literature Card navigation does not regress.
10. As a Session user, I want a missing `go <target>` target to retain its current one-line error and leave the Pointer unchanged, so that failed navigation remains safe.
11. As a Session user, I want help to show `go [<target>] | root`, so that the optional target and retained alias are discoverable.
12. As a TUI Session user, I want `go <target> <extra>` to fail with `usage: go [<target>]`, so that malformed navigation has a precise correction.
13. As a piped Session user, I want the same malformed-arity error, so that command parsing is consistent across Session paths.
14. As a shell user, I want `zt go` to remain unknown and absent from shell help, so that this Session-only feature does not imply a shell Pointer.
15. As a shell user, I do not want a `go --at` form, so that shell syntax is not expanded for a Session-only action.
16. As a shell user starting plain `zt`, I do not want a new initial-target option, so that Session startup remains unchanged at `ROOT`.
17. As a maintainer, I want bare `go` to reuse the existing Pointer-to-`ROOT` behavior, so that `go` and `root` cannot drift apart.
18. As a maintainer, I want target resolution unchanged, so that Locations, Citation keys, missing targets, and Broken links retain their current rules.
19. As a maintainer, I want PTY-backed coverage for the TUI command, so that the real raw-terminal input and redraw path proves the behavior.
20. As a maintainer, I want piped Session integration coverage, so that the non-TTY Session path proves the same command contract.
21. As a maintainer, I want regression coverage for help, malformed arity, shell exclusion, Location navigation, and Citation-key navigation, so that the narrow feature does not broaden accidentally.
22. As a maintainer, I want the complete Rust quality gate to pass, so that the feature lands without formatting, compilation, test, lint, or whitespace regressions.

## Implementation Decisions

- Both supported Session command handlers accept `go` with zero or one target.
- Zero targets use the same Pointer-to-`ROOT` transition as the retained `root` alias.
- One target continues through the existing target-existence and Card-navigation path.
- More than one target-like argument fails with the exact one-line text `usage: go [<target>]`.
- TUI root navigation clears the current status message whether invoked through `go` or `root`.
- Piped root navigation relies on the existing Session loop to redraw the resulting `ROOT` view.
- Root navigation is idempotent when the Pointer is already at `ROOT` and does not mutate Card data.
- Session help presents `go [<target>] | root` and otherwise retains the existing bare Session command list.
- Shell parsing and plain Session startup remain unchanged. No shell `zt go`, `--at` form, or initial-target option is introduced.
- Target syntax and resolution remain unchanged: a target is an existing Location or Citation key under the established navigation rules.
- No schema, storage, daemon, service-lifecycle, Link, editing, creation, deletion, or moving behavior changes.

## Testing Decisions

- Prefer the highest existing behavioral seams: PTY-backed workflows for the raw TUI Session and process-level piped-input workflows for the line Session.
- A TUI workflow moves to a Card, establishes a visible status message, enters bare `go`, and proves that the Pointer returns to `ROOT` with the message cleared.
- A TUI workflow invokes bare `go` while already at `ROOT` and proves it succeeds without an error or Card-data mutation.
- Piped Session workflows prove Card-to-`ROOT` and already-at-`ROOT` behavior through visible Session output.
- Both Session paths prove the exact `usage: go [<target>]` failure for extra arguments.
- Help coverage proves the exact navigation fragment `go [<target>] | root`.
- Existing Location and Citation-key navigation workflows remain regression authority for `go <target>`.
- Shell-level coverage proves `zt go` remains unknown and shell help does not advertise it.
- The final verification gate is `cargo fmt -- --check`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check`.

## Out of Scope

- A shell `zt go` command.
- Any shell `go --at` or `zt go --at <target>` form.
- An initial-Location or initial-target option for plain `zt`.
- Removing or changing the Session `root` alias.
- Changing Location or Citation-key syntax, validation, or resolution.
- Changing missing-target or Broken-link navigation behavior.
- Card storage, schema, Link, Reverse link, editing, creation, deletion, moving, daemon, or service-lifecycle changes.
- Unrelated Session command parsing or help redesign.

## Further Notes

- The locked source authority is `doc/session-bare-go-root-design.md`.
- Use the canonical terms `Session`, `Pointer`, `ROOT`, `Location`, `Citation key`, and `target` throughout implementation and verification.
- The work is intentionally KISS and is split into exactly two AFK-ready implementation slices.
- Every locked behavior and exclusion is assigned to at least one implementation issue; there is no separate horizontal testing issue.
