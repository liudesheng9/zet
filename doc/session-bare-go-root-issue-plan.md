# Session Bare `go` to `ROOT` Issue Plan

Parent PRD: [#40](https://github.com/liudesheng9/zet/issues/40)

Source design: [session-bare-go-root-design.md](session-bare-go-root-design.md)

Status: implemented and verified.

The feature is intentionally limited to exactly two AFK tracer-bullet issues. Each slice delivers observable Session behavior with its own automated proof; there is no separate horizontal testing issue.

## Published Issues

- [#40 [PRD] Bare Session `go` returns to `ROOT`](https://github.com/liudesheng9/zet/issues/40)
- [#41 Return both Session paths to `ROOT` with bare `go`](https://github.com/liudesheng9/zet/issues/41)
- [#42 Finish the bare-`go` command contract and regression gate](https://github.com/liudesheng9/zet/issues/42)

## Published Vertical Slices

1. **Return both Session paths to `ROOT` with bare `go`**
   - Type: AFK
   - Blocked by: None
   - User stories: 1-10 and 17-20
   - Delivers the complete valid-command path across TUI input, piped input, Pointer transition, redraw/message behavior, retained `root` alias, existing target navigation, and behavioral tests.

2. **Finish the bare-`go` command contract and regression gate**
   - Type: AFK
   - Blocked by: #41
   - User stories: 7-16 and 18-22
   - Delivers the complete discoverability and invalid-command path across Session help, malformed arity, shell exclusion, active documentation, regression coverage, and the final quality gate.

## Published Issue Specifications

### Slice 1

<!-- ISSUE_1_BODY_START -->
## Parent

- #40

## What to build

Make bare `go` return the Session Pointer to `ROOT` in both the raw terminal TUI and the piped line-session fallback. The transition must be the same behavior already exposed by the retained `root` alias: it is safe to invoke from a Card or from `ROOT`, it does not mutate Card data, and the TUI clears any current status message.

Keep `go <target>` unchanged. Existing Location and Citation-key targets must still navigate to their Cards, while a missing target must keep its existing one-line error and leave the Pointer unchanged.

The completed slice must be independently demonstrable in both Session paths and must reuse the established Pointer-to-`ROOT` behavior rather than creating a divergent root-navigation implementation.

## Acceptance criteria

- [ ] In a TUI Session viewing a Card, bare `go` moves the Pointer to `ROOT`.
- [ ] TUI bare `go` clears the current status message exactly like `root`; a workflow establishes a visible message before returning to `ROOT` and proves that stale message is gone.
- [ ] TUI bare `go` while already at `ROOT` succeeds as an idempotent no-op with no error and no Card-data mutation.
- [ ] In a piped Session viewing a Card, bare `go` moves the Pointer to `ROOT` and the normal Session loop redraws the `ROOT` view.
- [ ] Piped bare `go` while already at `ROOT` succeeds without an error or Card-data mutation.
- [ ] `root` remains supported in both Session paths and performs the same Pointer transition as bare `go`.
- [ ] `go <Location>` retains its existing successful navigation behavior.
- [ ] `go <Citation-key>` retains its existing successful navigation behavior.
- [ ] A missing `go <target>` target retains its existing one-line error and leaves the Pointer unchanged.
- [ ] The implementation reuses one root-navigation behavior for bare `go` and `root`; it does not change target resolution, Session startup, storage, Links, editing, creation, deletion, moving, or service lifecycle behavior.
- [ ] PTY-backed tests prove the real TUI input, status clearing, Card-to-`ROOT`, and already-at-`ROOT` paths.
- [ ] Process-level piped Session tests prove the matching line-session transitions and visible redraw behavior.

## Blocked by

None - can start immediately.
<!-- ISSUE_1_BODY_END -->

### Slice 2

<!-- ISSUE_2_BODY_START -->
## Parent

- #40

## What to build

Finish the public command contract for optional-target Session `go`. Session help must present `go [<target>] | root`, and both Session paths must reject more than one target-like argument with the exact one-line error `usage: go [<target>]`.

Prove and preserve the Session-only boundary: shell `zt go` remains unknown, shell help does not advertise it, no `go --at` form is added, and plain `zt` gains no initial-target option. Align the active product documentation with the locked optional-target contract while leaving closed issue archive documents unchanged.

Complete the feature with regression coverage for valid Location and Citation-key navigation, missing targets, the retained `root` alias, and every locked non-goal, then run the full Rust quality gate.

## Acceptance criteria

- [ ] Session help presents the navigation fragment exactly as `go [<target>] | root` and otherwise retains the existing bare Session command list.
- [ ] TUI Session input `go <target> <extra>` fails with exactly `usage: go [<target>]` and does not change the Pointer.
- [ ] Piped Session input `go <target> <extra>` fails with exactly `usage: go [<target>]` and does not change the Pointer.
- [ ] Shell `zt go` remains an unknown command, and shell help does not list `zt go`.
- [ ] No shell `go --at` or `zt go --at <target>` form is added.
- [ ] Plain `zt` gains no initial-Location or initial-target option and continues starting a Session at `ROOT`.
- [ ] Active product documentation describes bare `go`, `go <target>`, and the retained `root` alias consistently; closed issue archive documents are not revised.
- [ ] Regression coverage proves `root`, `go <Location>`, `go <Citation-key>`, and missing-target behavior remain as locked after the command-contract changes.
- [ ] The completed change remains limited to Session command routing, Session help, active contract documentation, and focused Session/shell-boundary tests; Card storage, Link resolution, editing, creation, deletion, moving, daemon, and service lifecycle behavior remain unchanged.
- [ ] `cargo fmt -- --check` passes.
- [ ] `cargo check --all-targets` passes.
- [ ] `cargo test --all-targets` passes, including PTY and piped Session coverage from the parent and Slice 1.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
- [ ] `git diff --check` passes.

## Blocked by

- #41
<!-- ISSUE_2_BODY_END -->

## Coverage Map

| Locked design area | PRD stories | Owning slice |
| --- | --- | --- |
| Session-only scope; no shell `zt go`, `--at`, or initial-target option | 14-16 | Slice 2 |
| Bare `go` moves the Pointer to `ROOT` in TUI and piped Sessions | 1-3, 6 | Slice 1 |
| Retained `root` alias and shared root-navigation behavior | 3, 7, 17 | Slice 1; regression in Slice 2 |
| TUI status clearing and already-at-`ROOT` idempotence | 4-5 | Slice 1 |
| Piped Session redraw and already-at-`ROOT` idempotence | 2, 4, 6 | Slice 1 |
| Existing Location, Citation-key, missing-target, and Broken-link rules | 8-10, 18 | Slice 1; regression in Slice 2 |
| Exact `go [<target>] | root` Session help | 11 | Slice 2 |
| Exact malformed-arity error in both Session paths | 12-13 | Slice 2 |
| Narrow implementation boundary and active-document alignment | 17-18 | Both slices; final audit in Slice 2 |
| PTY and piped Session behavioral coverage | 19-20 | Slice 1; full-suite rerun in Slice 2 |
| Help, shell-boundary, navigation regression coverage, and complete quality gate | 21-22 | Slice 2 |

## Publication Audit

- Exactly two implementation slices are specified; the parent PRD is not counted as an implementation slice.
- Both slices are AFK-ready.
- Slice 1 is independently runnable and demonstrable; Slice 2 is blocked only by #41.
- All 22 PRD stories are assigned to at least one slice.
- Every locked decision, guardrail, and acceptance item in the source design appears in the issue specifications or coverage map.
- PRD #40 and implementation issues #41-#42 carry `enhancement` and `ready-for-agent`.
- The live issue bodies use #40 as their Parent, and #42 uses #41 as its sole blocker.
- The live titles, bodies, labels, parent references, and dependency were re-read after publication with `gh`.
- No implementation is included in this planning work.

## Implementation Closeout

- Slice #41 closed on 2026-07-20 with [TDD and acceptance evidence](https://github.com/liudesheng9/zet/issues/41#issuecomment-5020198112).
- Slice #42 closed on 2026-07-20 with [command-contract and full-gate evidence](https://github.com/liudesheng9/zet/issues/42#issuecomment-5020288278).
- Parent PRD #40 closed on 2026-07-20 with [the complete 22-story audit](https://github.com/liudesheng9/zet/issues/40#issuecomment-5020295882).
- The final unchanged implementation passed formatting, all-target compilation, 121 automated tests, Clippy with warnings denied, and diff hygiene; one explicitly manual Windows clipboard smoke remains ignored by design.
