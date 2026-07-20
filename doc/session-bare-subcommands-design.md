# Session Bare Subcommands Design

Status: design locked for implementation.

## Grounded Facts

- `zt` is a Rust CLI-only local note system.
- Plain shell `zt` starts an interactive Session when the service is up.
- A Session has a per-session Pointer and a one-line command bar.
- The existing locked design says commands entered in the Session command bar must include the full `zt` prefix, such as `zt n` or `zt e`.
- The current implementation follows that older rule in both Session paths:
  - `cmd_line_session`, used when stdin or stdout is not a terminal.
  - `cmd_tui_session`, used when both stdin and stdout are terminals.
- Current Session handlers reject input whose first token is not `zt` with `session commands must start with `zt``.
- Current tests exercise full-prefix Session commands in both PTY-backed TUI tests and piped line-session tests.
- Shell commands are separate from Session command-bar commands. Shell pointer-dependent commands still require explicit arguments such as `zt e --at <location>`.

## Locked Decisions

- This design supersedes the older Session command-bar full-prefix rule.
- Inside any interactive Session, the user enters Session subcommands without typing the executable prefix.
- Examples:
  - `e` edits the current Card.
  - `n` creates the direct successor of the current Card.
  - `b` creates the next side successor of the current Card.
  - `go` moves the Session Pointer to `ROOT`.
  - `go 1/2` moves the Session Pointer to Location `1/2`.
  - `mv 1/2|a` moves the current Card subtree to Location `1/2|a`.
  - `q` exits the Session.
- The internal Session command entrance treats command-bar text as the subcommand tail and prepends the executable command token before dispatch.
- Because of that automatic prefixing, typing `zt xxx` inside a Session is interpreted as a request for a `zt` Session subcommand and is rejected.
- `zt xxx` inside a Session must not be treated as an alias for `xxx`.
- `zt xxx` inside a Session must not be forwarded to shell command execution.
- Bare Session subcommands must work in both Session paths:
  - TUI Session command bar.
  - Piped line-session fallback.
- Shell command syntax is unchanged outside a Session. The user still invokes the executable as `zt`, for example `zt e --at <location>`.
- Session commands continue to use the Session Pointer where the existing command contract says they do.
- Shell commands continue to avoid implicit Pointer use.
- This change does not alter Card, Topic, Location, Link, Reverse link, edit-lock, delete, move, or validation behavior.
- This change does not add command history or edit-caret movement to the command bar.
- Confirmation prompts remain data prompts, not Session commands. Delete still asks for `delete`; move still asks for `move`; topic deletion still asks for the Topic Location.
- The TUI command-bar prompt changes from `zt>` to `>` so the interface no longer suggests typing the executable prefix inside a Session.
- `help` inside a Session shows only bare Session forms, with navigation presented as `go [<target>] | root`.
- If a user types `zt e` inside a Session, the refusal text is `unknown session command: zt`.
- If a user types only `zt` inside a Session, the refusal text is also `unknown session command: zt`.
- The same refusal rule applies to other Session inputs whose first token is `zt`: the first token is interpreted as the requested subcommand name, not as a removable executable prefix.
- Existing PTY and line-session tests must be updated to use bare Session subcommands for Session input.
- Tests should also prove that a typed Session command beginning with `zt`, such as `zt e`, is refused rather than accepted as `e`.
- Acceptance requires both PTY-backed TUI coverage and piped line-session coverage for bare Session subcommands and `zt e` refusal.
- The active main design and PRD should be updated so they describe bare Session subcommands instead of the superseded full-prefix Session syntax.
- Implementation should introduce a small shared Session input normalizer used by both TUI and line-session paths.
- The shared normalizer owns only the translation from raw Session command-bar text to the internal prefixed dispatch shape; command execution handlers should otherwise stay close to their current structure.
- Shell `zt help` keeps showing shell syntax only.
- Session `help` keeps showing bare Session syntax only.
- The old error text `session commands must start with `zt`` should disappear entirely after implementation.
- Unknown Session input should report `unknown session command: <first-token>`.
- Blank or whitespace-only Session command input remains a no-op.
- Unknown Session command errors include only the first token. For example, Session input `foo bar` reports `unknown session command: foo`.
- Session command parsing trims leading and trailing whitespace before interpreting the first token and arguments.
- For example, Session input `  e  ` behaves like `e`.
- Session `t <title>` keeps the current title parsing behavior: the title is the rest of the line joined by spaces, and no shell-style quote parsing is added.
- Because Session `t <title>` does not use shell-style quote parsing, `t My Topic` creates title `My Topic`, while `t "My Topic"` includes the quote characters in the title.
- Session `t` with no title keeps the existing topic-title validation error rather than becoming an unknown-command or parser error.
- Implementation may mechanically update existing PTY and line-session tests from full-prefix Session input to bare input in the same change.
- Duplicate old-prefix workflow tests are not required. Keep old-prefix coverage only for explicit refusal cases such as `zt e`.

## Implementation Boundary

- Change only Session command parsing, Session help text, Session tests, and directly related user-facing command-bar copy.
- Do not change shell command parsing in `src/main.rs`.
- Do not change stored data shape or migration behavior.
- Do not change closed issue archive files unless a future task explicitly asks for archive text edits.

## Open Questions

None currently identified.
