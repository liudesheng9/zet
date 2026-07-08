## Parent

#1

## What to build

Implement read-only utility commands, help output, and command availability rules across service-down shell, service-up shell, and interactive Session contexts.

## Acceptance criteria

- [ ] `zt help` works as a shell command.
- [ ] `zt help` works inside the Session command bar.
- [ ] `zt help` shows only commands available in the current context.
- [ ] `zt stats` reports total Cards, Topic Cards, and regular Cards.
- [ ] `zt stats` is available inside the Session command bar.
- [ ] `zt status` is available inside the Session command bar.
- [ ] `zt version` prints only the CLI version.
- [ ] `zt lsbk` is available inside the Session command bar and as a shell command while the service is up.
- [ ] Shell read-only commands continue to return nonzero exit codes on failure.
- [ ] Service-down shell command availability remains limited to `zt up`, `zt status`, `zt version`, `zt config show`, and `zt config set archive_root <path>`.
- [ ] Service-up shell command availability includes configured shell commands from the PRD, including pointer-dependent commands with `--at`.
- [ ] The initial design includes no text search command.

## Blocked by

#10
