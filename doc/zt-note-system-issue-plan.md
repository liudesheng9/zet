# ZT Note System Issue Plan

Parent PRD: [#1](https://github.com/liudesheng9/zet/issues/1)

This breakdown is approved by the user instruction to approve the issues. Issues are ordered by dependency and intended as AFK-ready vertical slices.

## Published Issues

- [#1 [PRD] ZT CLI note system](https://github.com/liudesheng9/zet/issues/1)
- [#2 Bootstrap zt CLI, config, daemon, and SQLite storage](https://github.com/liudesheng9/zet/issues/2)
- [#3 Implement Location and Card text validation engine](https://github.com/liudesheng9/zet/issues/3)
- [#4 Implement shell Topic/Card creation and editing with --at](https://github.com/liudesheng9/zet/issues/4)
- [#5 Implement Link, Reverse link, and broken-link engine](https://github.com/liudesheng9/zet/issues/5)
- [#6 Implement interactive Session read/navigation views](https://github.com/liudesheng9/zet/issues/6)
- [#7 Implement Session command-bar writes and TUI editing](https://github.com/liudesheng9/zet/issues/7)
- [#8 Implement safe delete with verification and side-successor compaction](https://github.com/liudesheng9/zet/issues/8)
- [#9 Implement safe move with subtree relocation and link rewriting](https://github.com/liudesheng9/zet/issues/9)
- [#10 Implement service/session concurrency and operational safety](https://github.com/liudesheng9/zet/issues/10)
- [#11 Implement read-only utilities, help, and command availability](https://github.com/liudesheng9/zet/issues/11)

1. Bootstrap CLI, config, daemon, and SQLite storage
   - Type: AFK
   - Blocked by: None
   - Covers the runnable `zt` command, service lifecycle, configuration, database creation, WAL mode, logs, status, version, and storage table.

2. Implement Location and Card text validation engine
   - Type: AFK
   - Blocked by: Bootstrap CLI, config, daemon, and SQLite storage
   - Covers Location grammar, successor generation, title/body/reverse-link text parsing, and malformed Card validation.

3. Implement shell Topic/Card creation and editing with `--at`
   - Type: AFK
   - Blocked by: Location and Card text validation engine
   - Covers shell `zt t`, `zt n --at`, `zt b --at`, `zt e --at`, `$EDITOR`, temporary edit files, first-save cancellation, and script-friendly output.

4. Implement Link, Reverse link, and broken-link engine
   - Type: AFK
   - Blocked by: Shell Topic/Card creation and editing with `--at`
   - Covers link macros, generated Reverse links, new versus existing Broken links, and `zt lsbk`.

5. Implement interactive Session read/navigation views
   - Type: AFK
   - Blocked by: Link, Reverse link, and broken-link engine
   - Covers plain `zt`, ROOT view, Topic view, regular Card view, command bar, `zt root`, `zt go`, `zt ls`, link activation, and read-only display behavior.

6. Implement Session command-bar writes and TUI editing
   - Type: AFK
   - Blocked by: Interactive Session read/navigation views
   - Covers Session `zt t`, `zt n`, `zt b`, `zt e`, TUI edit mode, pointer use, cancel behavior, and shared validation/save behavior.

7. Implement safe delete with verification and side-successor compaction
   - Type: AFK
   - Blocked by: Link, Reverse link, and broken-link engine
   - Covers regular delete, Topic delete, confirmation prompts, verification charts, broken links, pointer movement, and side-successor compaction.

8. Implement safe move with subtree relocation and link rewriting
   - Type: AFK
   - Blocked by: Link, Reverse link, and broken-link engine
   - Covers `zt mv`, destination validation, move charts, subtree rewriting, link rewriting, reverse-link regeneration, and pointer movement.

9. Implement service/session concurrency and operational safety
   - Type: AFK
   - Blocked by: Interactive Session read/navigation views
   - Covers Session registration, `zt down` refusal, daemon disconnect handling, global edit lock, read-only behavior during edits, and edit-lock failures.

10. Implement read-only utilities, help, and command availability
    - Type: AFK
    - Blocked by: Service/session concurrency and operational safety
    - Covers `zt help`, `zt status`, `zt stats`, `zt version`, command availability by service/session state, and nonzero exit-code behavior.
