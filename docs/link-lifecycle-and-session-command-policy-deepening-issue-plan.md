# Link Lifecycle and Session Command Policy Deepening Issue Plan

Parent PRD: [#43](https://github.com/liudesheng9/zet/issues/43)

Source design: [link-lifecycle-and-session-command-policy-deepening-design.md](link-lifecycle-and-session-command-policy-deepening-design.md)

Source PRD: [link-lifecycle-and-session-command-policy-deepening-prd.md](link-lifecycle-and-session-command-policy-deepening-prd.md)

Status: implemented, verified, and closed on 2026-07-22.

The plan uses six AFK tracer-bullet issues. Each slice moves a complete observable path through a deepened module and the existing CLI or PTY acceptance seam. There is no horizontal parser-only, adapter-only, or test-only issue.

## Approved Vertical Slices

1. **Use one Link interpretation for Card saves and TUI activation**
   - Type: AFK
   - Blocked by: None
   - User stories covered: 1-26, 40-42, 82-87, and 91-92
   - Delivers the canonical Link body interpretation through focused tests, Card save validation, TUI classification, styling, activation, and malformed-text compatibility.

2. **Rewrite moved and renamed Link targets through the Link module**
   - Type: AFK
   - Blocked by: #44
   - User stories covered: 1-8, 31-36, 40-42, 82-89, and 91-92
   - Delivers exact body-only rewrites and occurrence-count previews for Location moves and Citation-key renames while preserving atomic storage behavior.

3. **Derive Reverse links and Broken link reports through the Link module**
   - Type: AFK
   - Blocked by: #45
   - User stories covered: 1-8, 21-42, 81-92
   - Delivers Reverse link derivation, structured Broken link results, shell/TUI `lsbk`, delete consequences, final scanner deletion, and the completed deep Link module.

4. **Run navigation and read-only Session commands through one policy**
   - Type: AFK
   - Blocked by: #46
   - User stories covered: 1-2, 43-60, 66-71, 78-90, and 91-92
   - Delivers one Session command execution interface and Session state owner through both real adapters for parsing, navigation, help, lists, statistics, status, Broken links, exit, errors, formatting, and redraw.

5. **Run Session creation and editing through the shared interaction seam**
   - Type: AFK
   - Blocked by: #47
   - User stories covered: 1-2, 43-47, 59-63, 72-73, 76-87, and 91-92
   - Delivers Topic, Regular Card, and Literature Card creation and non-destructive editing through one command policy while retaining the two editor adapters and safe cancellation.

6. **Run destructive Session commands through one policy and finish the architecture gate**
   - Type: AFK
   - Blocked by: #48
   - User stories covered: 1-2, 43-47, 56-66, 74-77, and 81-92
   - Delivers delete, move, Citation-key rename confirmation, final Pointer outcomes, duplicate-handler deletion, complete regressions, documentation consistency, and the full quality gate.

## Approved Issue Specifications

### Slice 1

<!-- ISSUE_1_BODY_START -->
## Parent

- #43

## What to build

Introduce one canonical Link interpretation and carry it through two complete behaviors: Card save validation and TUI Link rendering/activation. The Link module must interpret Link occurrences and exact source ranges from Card bodies, classify targets against immutable Card snapshots, and preserve the current old-versus-new Broken target rule without opening SQLite or introducing a storage trait.

Characterize malformed stored Link text and repeated Broken targets before replacing current scanners. Supported writes must retain their exact validation behavior. TUI reading must continue showing stored Card bodies, valid Links must remain highlighted and clickable, Broken links must remain visibly Broken and non-clickable, and malformed fragments must not become clickable. Terminal geometry, wrapping, colours, and mouse hit spans remain owned by the TUI adapter.

This slice is complete only when Card save validation and TUI rendering use the same Link occurrence interpretation and both focused and end-to-end tests prove no observable behavior changed.

## Acceptance criteria

- [ ] One crate-private Link module owns canonical Card-body Link syntax, target text, and exact source ranges.
- [ ] The Link module receives immutable Card snapshots containing only the address, title, and body data required by Link behavior.
- [ ] The Link module does not open SQLite, own transactions or the edit lock, or introduce a storage trait.
- [ ] `[[target]]` remains the only Link syntax; display text remains unsupported.
- [ ] Targets remain valid Locations or case-significant Citation keys under the current grammar.
- [ ] Only the client-authored Card body contributes outbound Links; Card titles, BibTeX metadata, and generated Reverse link sections remain excluded.
- [ ] Card save validation uses the Link module and retains exact malformed-macro and invalid-target errors.
- [ ] A new Link to a missing target is rejected, while an already-Broken target may remain under the current target-set semantics.
- [ ] Characterization tests record repeated Broken targets and the current malformed stored-text behavior before scanners move.
- [ ] TUI rendering uses Link occurrences and validity from the Link module without moving terminal geometry, soft wrapping, styling, or mouse hit-span calculation into that module.
- [ ] Valid Links remain highlighted and clickable; Broken links remain visibly Broken and non-clickable.
- [ ] TUI and piped reading continue showing stored Card bodies without making malformed fragments clickable; strict Link operations fail clearly and write nothing.
- [ ] No automatic Link repair is added.
- [ ] Focused Link-interface tests cover syntax, source ranges, multiple occurrences, target grammar, valid/Broken classification, malformed text, and old-versus-new Broken targets.
- [ ] Existing CLI save-validation and PTY Link-rendering/activation workflows pass without user-visible changes.
- [ ] The slice adds no schema, Link syntax, Location topology, Card mutation, Session policy, shell, configuration, recovery, or telemetry change.

## Blocked by

None - can start immediately.
<!-- ISSUE_1_BODY_END -->

### Slice 2

<!-- ISSUE_2_BODY_START -->
## Parent

- #43

## What to build

Move every Location-move and Citation-key-rename Link rewrite path through the canonical Link module. The completed behavior must identify actual Link target occurrences in client-authored Card bodies, report the same occurrence counts in confirmation previews, and rewrite only mapped targets while preserving every byte outside each target range.

Storage remains responsible for the existing atomic move or rename transaction. Card titles, BibTeX metadata, plain text that merely resembles a target, and generated Reverse link sections must not be rewritten as Link sources. Citation key case and all confirmation behavior remain unchanged.

This slice is independently demonstrable through a Regular Card subtree move and a Literature Card Citation-key rename that both update inbound Links and leave all non-Link text exact.

## Acceptance criteria

- [ ] Location moves obtain Link occurrence counts and rewritten Card bodies from the Link module.
- [ ] Citation-key renames obtain Link occurrence counts and rewritten Card bodies from the same module.
- [ ] Rewrite previews retain occurrence-count semantics, including repeated occurrences in one Card.
- [ ] Only actual `[[target]]` target ranges in client-authored Card bodies are rewritten.
- [ ] Every byte outside a rewritten target remains exact, including punctuation, whitespace, line endings, titles, and unrelated body text.
- [ ] Card titles, BibTeX metadata, and generated Reverse link sections are not scanned as outbound Link sources or rewritten as targets.
- [ ] Citation key case remains exact and case-only collision behavior remains unchanged.
- [ ] Link rewriting remains inside the caller's existing atomic Location-move or Citation-key-rename transaction.
- [ ] A failed parse, validation, confirmation, or write leaves all Card data unchanged.
- [ ] Focused Link-interface tests cover Location and Citation-key mappings, multiple mappings, repeated occurrences, exact byte preservation, malformed input, and no-match behavior.
- [ ] CLI integration tests prove exact rewrite counts, body-only rewrites, transaction safety, and unchanged move/rename confirmation output.
- [ ] PTY regressions prove moved and renamed targets remain navigable and Pointer outcomes remain unchanged.
- [ ] No storage trait, schema change, new Link syntax, broad Card mutation refactor, or user-visible behavior change is introduced.

## Blocked by

- #44
<!-- ISSUE_2_BODY_END -->

### Slice 3

<!-- ISSUE_3_BODY_START -->
## Parent

- #43

## What to build

Complete the deep Link lifecycle by deriving Reverse link sections and structured Broken link results from the canonical cross-Card interpretation. Route shell `zt lsbk`, TUI Session `lsbk`, save/delete/move/rename Reverse link regeneration, and delete-created Broken links through those results while retaining every existing output and transaction rule.

The Link module owns deterministic Reverse link wording and ordering, one Reverse link line per source Card and target, omission of Broken targets, and Broken link records containing source address, source title, target, and source line. Shell and TUI adapters continue formatting those records differently.

After every consumer has moved, delete all independent production Link scanners. The completed slice must prove through repository-wide searches that raw Link macro scanning exists only in the Link module, tests, and literal user-facing text.

## Acceptance criteria

- [ ] Reverse link sections are derived by the Link module from immutable Card snapshots.
- [ ] Duplicate Links from one source Card to one target create exactly one Reverse link line.
- [ ] Broken links create no Reverse link line.
- [ ] Reverse link wording and deterministic source ordering remain exact.
- [ ] Save, delete, Location move, and Citation-key rename apply returned Reverse link sections inside their existing atomic transactions.
- [ ] Deleting a target preserves inbound Link macros and makes them Broken without partial Card changes.
- [ ] Structured Broken link results retain source address, source title, target, and the matching source line.
- [ ] Shell `zt lsbk` formats the structured results with its current line-oriented output.
- [ ] TUI Session `lsbk` formats the same results with its current joined message and `no broken links` behavior.
- [ ] Broken Location and Citation-key targets retain identical classification rules and exact-case behavior.
- [ ] Strict operations on malformed stored Link text fail clearly and write nothing; reading remains useful and no automatic repair is introduced.
- [ ] Focused tests cover Reverse link deduplication, Broken omission, wording, ordering, source lines, duplicate Broken occurrences, deletion consequences, and malformed input.
- [ ] Existing CLI and PTY workflows prove Reverse links, `lsbk`, Link rendering, deletion, move, Citation-key rename, and Pointer behavior remain unchanged.
- [ ] All independent production Link scanners are deleted only after their consumers use the Link module.
- [ ] A repository-wide audit finds no raw Link macro scanner outside the Link module, tests, or literal user-facing text.
- [ ] The completed Link deepening adds no structural Link table, cached SQLite Link index, storage trait, schema migration, or unrelated refactor.

## Blocked by

- #45
<!-- ISSUE_3_BODY_END -->

### Slice 4

<!-- ISSUE_4_BODY_START -->
## Parent

- #43

## What to build

Introduce one Session command execution interface and one Session state owner, then carry navigation and read-only commands through both the TUI and piped adapters. The shared command module owns bare-input normalization, command matching, arity, preconditions, errors, Pointer transitions, continuation/exit, and structured results.

The TUI and piped paths remain separate real adapters. Their command input, result formatting, message handling, and redraw behavior stay distinct, but they must no longer interpret command strings or assign Pointer outcomes independently. The interaction seam must stay small and must not expose one method per command.

This slice covers blank input, unknown commands, `q`, bare `go`, `root`, `go <target>`, `ls`, `stats`, `status`, `lsbk`, and `help` end to end through both adapters while preserving shell separation.

## Acceptance criteria

- [ ] One crate-private Session command module exposes one command-execution entry point.
- [ ] One Session state owner controls Pointer changes; adapters may read but do not independently assign command Pointer outcomes.
- [ ] TUI and piped Sessions remain separate adapters at one real interaction seam.
- [ ] The seam does not expose one method per Session command.
- [ ] Bare input, whitespace trimming, blank no-op behavior, and `unknown session command: <first-token>` remain exact.
- [ ] Bare `go`, `root`, `go <Location>`, `go <Citation-key>`, missing targets, and extra-argument `usage: go [<target>]` behavior remain exact.
- [ ] Failed navigation and other failed commands leave the Pointer unchanged.
- [ ] `q` retains its current successful Session exit behavior.
- [ ] `ls`, `stats`, `status`, `lsbk`, and `help` return structured semantic results from the shared command module.
- [ ] TUI formatting remains compact, including its current `stats`, `status`, and `lsbk` message forms.
- [ ] Piped formatting remains line-oriented, including its current detailed `status` and shell-style `lsbk` behavior.
- [ ] TUI message clearing and retention remain command-for-command identical to current behavior.
- [ ] Piped Sessions print the view after successful commands and retain current no-redraw behavior after errors.
- [ ] Shell parsing and output remain unchanged; shell `zt go` stays unavailable and shell commands gain no Pointer.
- [ ] Focused tests use scripted TUI-like and piped-like adapters to prove identical command meaning and distinct formatting/redraw outcomes.
- [ ] Existing process-level piped tests and PTY workflows pass for every command in this slice.
- [ ] No TUI event-loop merge, new TUI framework, Card mutation refactor, new command, flag, or user-visible behavior is introduced.

## Blocked by

- #46
<!-- ISSUE_4_BODY_END -->

### Slice 5

<!-- ISSUE_5_BODY_START -->
## Parent

- #43

## What to build

Move Session creation and non-destructive editing through the shared command policy and interaction seam. The shared module owns command meaning, Card preconditions, edit selection, storage orchestration, cancellation outcomes, and Pointer transitions for Topic, Regular Card, and Literature Card workflows. The TUI and piped adapters retain their genuinely different editors and presentation.

Cover `t <title>`, `l`, `n`, `b`, and `e` through both adapters. Preserve non-shell title parsing, TUI `EditorModel`, mouse and clipboard behavior, the piped byte-oriented editor, Literature Card edit-part selection, validation retries, edit locking, transactions, and safe cancellation. Literature metadata editing that does not rename the Citation key completes in this slice; rename confirmation completes with the destructive interaction paths in the next slice.

This slice is independently demonstrable by creating and editing each Card kind in both Session paths with unchanged Pointer outcomes.

## Acceptance criteria

- [ ] `t <title>`, `l`, `n`, `b`, and `e` command meaning is owned by the shared Session command module.
- [ ] `t <title>` retains whitespace joining and does not add shell-style quote parsing.
- [ ] Command preconditions at `ROOT`, a Location, and a Citation key remain exact for every creation and edit command.
- [ ] The interaction seam exposes Card-text editing, Literature metadata editing, and Literature edit-part choice without one method per command.
- [ ] TUI editing retains `EditorModel`, mouse selection, clipboard, save/cancel keys, validation display, and current redraw behavior.
- [ ] Piped editing retains the existing byte-oriented save/cancel protocol, output, and validation retry behavior.
- [ ] Literature Card numbered edit selection and invalid-choice retry remain unchanged.
- [ ] Edit locks, Card validation, SQLite transactions, and Reverse link regeneration remain unchanged.
- [ ] Successful Topic, Regular Card, and Literature Card creation retain their current Pointer destinations.
- [ ] Successful non-renaming edits retain their current Pointer destinations and reading views.
- [ ] A canceled creation or edit leaves Card data and Pointer state unchanged under the existing contract.
- [ ] A failed validation leaves storage unchanged and keeps the same editable content available for correction.
- [ ] Shared Session command tests use scripted TUI-like and piped-like editing adapters to prove identical command meaning and safe outcomes.
- [ ] Existing process-level piped and PTY workflows prove real creation, editing, retries, cancellation, mouse, clipboard, and Pointer behavior.
- [ ] Shell creation/edit syntax and output remain unchanged and no Pointer state is added to shell commands.
- [ ] The slice adds no new editor, TUI framework, command, schema, configuration, recovery behavior, or broad Card mutation refactor.

## Blocked by

- #47
<!-- ISSUE_5_BODY_END -->

### Slice 6

<!-- ISSUE_6_BODY_START -->
## Parent

- #43

## What to build

Complete Session command consolidation by moving delete, move, and Citation-key rename confirmations and outcomes through the shared command policy and interaction seam. Preserve every destructive verification line, confirmation word, modal or line-oriented interaction, transaction, failure outcome, and Pointer transition.

After both adapters use one command policy for every Session command, delete the duplicated command match blocks. Finish the architecture change with complete focused, CLI, piped, and PTY regressions; repository-wide duplicate-implementation searches; active-document consistency; and the full Rust quality gate.

This slice is the final independently verifiable path: a TUI and piped Session can move and delete Cards and rename a Citation key through their existing confirmations while one shared command module owns the meaning and Pointer outcomes.

## Acceptance criteria

- [ ] `del`, `mv <new-location>`, and Citation-key rename confirmation are owned by the shared Session command module.
- [ ] The interaction seam exposes confirmation behavior without one method per destructive command.
- [ ] TUI confirmation modals retain their exact verification text, redraw behavior, mouse selection, clipboard behavior, and accepted confirmation words.
- [ ] Piped confirmations retain their exact line-oriented prompts, verification text, accepted words, and cancellation behavior.
- [ ] Edit lock, validation, delete/move/rename plans, SQLite transactions, Link rewriting, and Reverse link regeneration remain unchanged.
- [ ] Failed or canceled destructive commands leave Card data and the Pointer unchanged under the existing contract.
- [ ] Successful deletion retains the current parent-or-`ROOT` Pointer outcome for every Card kind.
- [ ] Successful move retains the current Pointer-to-new-Location outcome.
- [ ] Successful Citation-key rename retains the Pointer-to-new-Citation-key outcome.
- [ ] `q`, Ctrl+C, raw-mode cleanup, and disconnect behavior retain their current outcomes.
- [ ] TUI message clearing and piped success/error redraw behavior remain exact for the destructive paths.
- [ ] Both old duplicated Session command match blocks are deleted only after every command uses the shared module.
- [ ] A repository-wide audit finds one production Session command matcher and no adapter-owned Pointer transition policy.
- [ ] Focused Session command tests cover destructive success, failure, cancellation, confirmation, and Pointer outcomes through scripted adapters.
- [ ] Existing CLI, piped Session, and PTY workflows pass for create, edit, rename, delete, move, confirmation redraw, mouse, clipboard, raw mode, and disconnect behavior.
- [ ] Active documentation is consistent with the new ownership and preserves all user-visible contracts.
- [ ] `cargo fmt -- --check` passes.
- [ ] `cargo check --all-targets` passes.
- [ ] `cargo test --all-targets` passes, including all focused, CLI, piped, and PTY tests from earlier slices.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
- [ ] `git diff --check` passes.
- [ ] No schema, new Link syntax, new command, flag, configuration, recovery state, telemetry, Location topology, broad Card mutation, broad Session reading-projection, or TUI framework change is included.

## Blocked by

- #48
<!-- ISSUE_6_BODY_END -->

## Locked-Design Coverage Audit

| Locked design area | PRD representation | Owning slice |
| --- | --- | --- |
| Goal and behavior preservation | Problem, Solution, stories 1-2, Implementation Decisions | All slices; final proof in Slice 6 |
| Source authority and precedence | Further Notes, stories 78-80 and 90 | Slice 4 preserves shell/Session contracts; Slice 6 audits active docs |
| Existing domain language | Further Notes and canonical terminology throughout | All slices |
| Link current friction and deletion test | Problem, stories 3-5 and 40 | Slices 1-3; scanner deletion in Slice 3 |
| Link module responsibility and exclusions | Stories 3-8, 13-16, 26, 40-41; Implementation Decisions | Slice 1 establishes ownership; Slices 2-3 complete consumers |
| Link interface shape and immutable snapshots | Stories 4-8 and 41 | Slice 1 |
| Link syntax and target identity | Stories 9-16 | Slice 1 |
| Malformed stored text compatibility | Stories 17-20 and 82 | Slice 1 characterization; strict-operation regression in Slice 3 |
| New versus existing Broken links | Stories 21-23 | Slice 1; final lifecycle proof in Slice 3 |
| TUI Link rendering and hit spans | Stories 24-26 | Slice 1 |
| Reverse link derivation | Stories 27-32 | Slice 3 |
| Link rewriting and preview counts | Stories 33-36 | Slice 2 |
| Structured Broken link results and formatting | Stories 37-39 | Slice 3 |
| Session current friction and deletion test | Stories 43-47 and 84 | Slices 4-6; handler deletion in Slice 6 |
| Session command module responsibility | Stories 43-47, 67, and 77 | Slice 4 establishes; Slices 5-6 complete command classes |
| Bare syntax, errors, and navigation | Stories 48-58 | Slice 4 |
| Pointer state and outcomes | Stories 45 and 59-65 | Slice 4 navigation; Slice 5 create/edit; Slice 6 destructive paths |
| Session exit and disconnect | Story 66 | Slice 4 for `q`; Slice 6 final Ctrl+C/disconnect regression |
| Structured read results and adapter formats | Stories 67-71 | Slice 4 |
| TUI and piped editor adapters | Stories 72-73 | Slice 5 |
| Confirmation adapters | Stories 74-76 | Slice 6; rename setup in Slice 5 |
| Storage, shell, and Pointer separation | Stories 77-80 | Slices 4-6 |
| Dependency order and incremental migration | Stories 81-84 | All slices through the dependency chain |
| Focused, CLI, piped, and PTY testing | Stories 41-42 and 85-87; Testing Decisions | Every slice; final rerun in Slice 6 |
| Duplicate searches, quality gate, and docs | Stories 88-90 | Slice 3 for Link; Slice 6 for Session and final gate |
| Non-goals and KISS scope | Stories 91-92; Out of Scope | Every slice; final audit in Slice 6 |
| Open decisions | Further Notes records six approved AFK slices | All slices are ready without HITL decisions |

Every section and bullet group in the source design maps to numbered PRD stories, implementation or testing decisions, and at least one child issue. No architecture requirement exists only in the source design.

## Story Coverage Map

| User stories | Primary owning slice |
| --- | --- |
| 1-2 | All slices; final proof in Slice 6 |
| 3-26 | Slice 1 |
| 27-30 | Slice 3 |
| 31-36 | Slice 2 |
| 37-42 | Slice 3, with focused foundations in Slices 1-2 |
| 43-60 | Slice 4 |
| 61-63 | Slice 5 |
| 64-65 | Slice 6 |
| 66-71 | Slice 4, with final lifecycle regression in Slice 6 |
| 72-73 | Slice 5 |
| 74-76 | Slice 6, with rename editing prepared in Slice 5 |
| 77-80 | Slices 4-6 |
| 81-84 | Enforced by the full dependency chain |
| 85-87 | Every slice; final rerun in Slice 6 |
| 88-90 | Slice 3 for Link and Slice 6 for Session/final audit |
| 91-92 | Every slice; final scope audit in Slice 6 |

## Dependency Graph

```text
Slice 1: save + TUI Link interpretation
  -> Slice 2: move/rename Link rewrites
     -> Slice 3: Reverse/Broken results + scanner deletion
        -> Slice 4: navigation/read-only Session policy
           -> Slice 5: Session creation/editing
              -> Slice 6: destructive policy + final gate
```

## Approval Gate

- The user explicitly approved the issue design in the request that invoked `to-prd` and `to-issues`.
- All six slices are AFK: the source design has no open decisions and every slice has observable acceptance criteria.
- Dependencies follow the locked Link-first implementation order.
- Each slice is a vertical path through a module interface and a real CLI or PTY behavior.
- No issue is a horizontal parser-only, adapter-only, documentation-only, or test-only slice.
- Every one of the 92 PRD stories is assigned to at least one slice.
- Every source-design section appears in the locked-design coverage audit.
- Parent and child issues must carry `enhancement` and `ready-for-agent`.
- No implementation is included in this planning work.

## Completion Audit

- Parent PRD [#43](https://github.com/liudesheng9/zet/issues/43) and implementation issues [#44](https://github.com/liudesheng9/zet/issues/44) through [#49](https://github.com/liudesheng9/zet/issues/49) were closed as completed after implementation and verification evidence was added to each issue.
- The six slices were completed in the locked dependency order: Link interpretation, Link rewriting, Reverse and Broken link derivation, Session reads and navigation, Session creation and editing, then destructive Session policy and final audit.
- `src/link.rs` is the only production raw Link scanner. `src/session/command.rs` is the only production Session command matcher and owner of command-driven Pointer transitions. The old scanner and duplicated handler names are absent from production and test source.
- No schema, storage trait, new syntax, new command, shell-policy change, recovery state, or release change was introduced.
- `cargo fmt -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, and serialized `cargo test --all-targets -- --test-threads=1` passed.
- The serialized test result was 68 unit, 56 CLI/service, and 26 automated PTY tests passed. One explicitly manual real-Windows-clipboard smoke remained ignored by design.
