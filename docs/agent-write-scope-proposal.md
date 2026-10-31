# Write scope in the Write gate

Status: proposal (2026-10-31). Rounds 1 and 2 of the design are decided; round 3 is open (see the end).
Origin: the architecture review of the seam between agents and `zt mcp` (candidate 1 of 5).
It builds on `docs/literature-only-digest-design.md`, which introduced the Literature scope.

## The problem

An agent write passes two kinds of checks today.

| Check | Where it runs | Which clients it covers |
|---|---|---|
| Access (agent Workspace only) | server, `ops::*_tx` | every MCP client |
| Tree Locks | server, Write gate | every MCP client |
| Card check (`card_check`) | server, Write gate | every MCP client |
| Literature scope | client, `zt_check::mcp::Zt::request` behind a process-wide flag that `zt-digest` sets (`tools/zt-digest/src/main.rs:163`) | `zt-digest` only |
| Allow list (`-Allow`, `-Beneath`) | client, two copies in `tools/zt-digest/src/calls.rs` (single calls and the identified path) | `zt-digest call` only |
| Topic title gate (`-TopicGate`) | client, file lock plus bigram similarity in `calls.rs` | `zt-digest call -Open` only |
| `append_card` | client, read then edit with no base check | `zt-digest call` only |

The first three checks hold for every route. The last four hold only for the route a client chose.
This has three effects:

1. **The Literature-only rule rests on a prompt.** A DeepSeek job leaves the Literature scope by
   adding `-Open` to its helper command. Only the Haiku manager's approval rule `ZD_NO_OPEN`
   (`scripts/book-digest/workflow/modules/zt-digest-lib.js:86`) refuses that command.
2. **A reads-only job could write.** The single-call allow check never looked at `apply_batch`, so
   an empty allow list (the reads-only file of Organize and idea-grow) passed a batch of creates and
   edits. Fixed in zt-digest 0.37.12 as a stopgap (see Decision 3).
3. **Direct clients get none of it.** Claude Code and Codex call `mcp__zt__*` without any scope,
   allow list or Topic gate. The concept gate's own comment says "a raw Owner or GUI Topic creation
   is still possible".

## Decisions (round 1, 2026-10-31)

**D1. The scope is containment for workflow jobs and a guard-rail for hand work.** A workflow stage
fixes the scope of the jobs it starts, and a job cannot widen it. Hand work in Claude Code or Codex
declares its own scope and is not forced through a grant.

**D2. What moves into the server.**

| Check | Decision |
|---|---|
| Literature scope | Moves into the Write gate. Checked per item inside the write transaction, so `apply_batch` is covered by construction. |
| Allow list, with `-Beneath` | Moves into the Write gate with the Literature scope, as one rule. |
| Topic title gate | Moves into `create_topic` for agent Access: an exact or near-duplicate agent Topic title is refused for every client. The edit lock makes the check atomic, so the file lock goes away. |
| Concept gate | Stays in `zt-digest`. It needs a judge model and a workflow; it is not a fixed rule. |
| `append_card` | Stays a helper convenience, but becomes an `edit_card` with a base, so the server's existing conflict check covers it. The read-then-edit race goes away. |

**D3. Close the `apply_batch` gap at once, apart from the redesign.** Done in zt-digest 0.37.12:
`allow_refusal` (`tools/zt-digest/src/calls.rs`) checks every batch item as its own call, an `at`
of `@@name` names an earlier item of the same batch, the Cards a batch writes join the list, and a
batch preview passes like a read. A `create_successor` with `before` is refused under `-Allow`,
because it renumbers Side successors the list does not name.

## Decisions (round 2, 2026-10-31)

**D4. A job gets its scope from a launch flag.** `zt mcp` takes `--scope literature|open`, with
`--allow <file>` and `--beneath <file>` for the allow rule. `zt-digest` starts its `zt mcp` with the
scope its subcommand needs (today's `literature_scope` and `OPEN_TOOLS` in
`tools/zt-digest/src/main.rs` decide it). The helper `zt-digest call` loses its job-facing
`-Open`, so a job has no flag to reach for. This contains job mistakes, not a hostile job that
launches `zt mcp` by itself; the manager approvals still refuse that command. A grant recorded in
the archive (a token per job) stays possible later on the same seam.

**D5. The refusal is a typed `scope_refused`.** `{error: "scope_refused", message, tool, at, scope,
reason}`, plus `item {index, tool, op_key}` inside a batch. It travels like `card_locked`: as
`isError: true` with `structuredContent`, mapped beside the other typed refusals. Clients treat it
as a planning stop, never as a fault to retry. A batch preview reports it as one `refusals` entry
and keeps checking the independent items.

**D6. One allow rule, in the server.** A Card may change when it is listed, when this session made
it, or when it is the result of an earlier `@@` reference in the same request. A Card listed with
`--beneath` takes only a new Side successor. `create_topic`, `create_literature`,
`edit_literature_metadata`, `move_card`, `insert_card`, `delete_card` and a `create_successor` with
`before` are refused under an allow list. The single-call rule and the identified rule in
`calls.rs` become this one rule.

**D7. The Topic title rule refuses.** For agent Access, `create_topic` refuses a title whose key
(spaces, punctuation and symbols removed, lower case) equals an agent Topic's key, or whose
character-bigram similarity with one is at least the threshold. The refusal names that Topic. The
threshold is archive metadata, `gate.topic_similarity`, default 0.8. The check runs under the edit
lock, so it is atomic and the file lock (`-TopicGate`) goes away. The owner's GUI is not affected.
The concept gate (judge-based) stays in `zt-digest` and runs before the call.

**D8. `append_card` uses the single `edit_card` raw `base`**, the text the helper just read, so a
write between the read and the edit is refused as "changed since it was opened". The fingerprint
mismatch (no MCP read returns the value `base_fingerprint` expects: `card::authored_fingerprint`
vs `batch::fingerprint`) is a separate fix.

## Design sketch

```text
client (Claude, zt-digest call, zt.mjs, GUI)
   │ starts zt mcp with --scope literature|open [--allow f] [--beneath f]  (D4)
   ▼
zt mcp ── tools/call ──► ops::*_tx
                           └─ Write gate: Access · Tree Lock · Card check · Write scope
                                                   (Literature or open, optional allow set)
```

- The Write scope is a property of one `zt mcp` session. The server keeps the Cards it wrote in this
  session, so "the Cards this run made" needs no client bookkeeping.
- The check runs in the same place as the Lock check, inside the write transaction, once per item.
  A batch preview reports a scope refusal as one `refusals` entry, as it does for `card_locked`.
- The GUI acts with human Access and has no Write scope. Nothing changes for the owner.
- The client copies (`zt_check::mcp::scope`, both allow-list checks, `lock_gate`, the process-wide
  flag) are deleted once every caller declares its scope to the server.

## Open questions (round 3)

Each has a recommendation; the owner confirms or changes it.

1. **The default scope of a `zt mcp` that declares none.** Recommended: **open**, today's behaviour.
   Every job route goes through `zt-digest`, which always declares a scope, so a literature
   default adds no containment. An open default keeps Claude Code and Codex hand work, the 32 Rust
   test files that write through `zt mcp`, and `scripts/book-digest/copy-tree.mjs` working
   unchanged.
2. **The command that replaces the helper's `-Open`.** Today `-Open` is used by hand work in a
   harness with no MCP tools (`skills/zt/SKILL.md`), by `scripts/verify-release.ps1`, and by the
   test-only `scripts/book-digest/zt-calls.ps1`. Recommended: a separate subcommand,
   `zt-digest open-call`, with the same options as `call`, on no workflow's approval list. The
   approval rule `ZD_NO_OPEN` then refuses `open-call` instead of a flag.
3. **Rollout order.** Recommended, three releases, each deployed and verified before the next:
   - **P1 — server (zt minor release).** `zt mcp --scope/--allow/--beneath`, the session's made
     Cards, the scope check per item in the write transaction, `scope_refused`, the Topic title
     rule with `gate.topic_similarity`. The default stays open, so no client changes behaviour.
     Tests in `tests/workspaces_mcp.rs` through a real `zt mcp`.
   - **P2 — clients (zt-digest minor release).** `zt_check::mcp::Zt::start` passes the scope and
     the allow files; `scope_refused` stops a stage like `card_locked`; `append_card` sends a
     `base`; `open-call` replaces `-Open`. Then delete the client copies: `literature_refusal`
     and the process-wide flag, `allow_refusal` and the identified allow check, `lock_gate` and
     `-TopicGate`. P2 needs a P1 `zt` on PATH; an older `zt mcp` refuses the new arguments.
   - **P3 — workflows and skills (blueprint patch releases, reinstall).** `ZD_NO_OPEN` names
     `open-call`; `skills/zt/SKILL.md`, the model skills' helper facts and `INSTRUCTIONS` name
     the Write scope and `scope_refused`. Keep `ZD_NO_OPEN` until P3 lands.
4. **Tracking.** Recommended: one PRD issue for this proposal and one issue per phase in
   `liudesheng9/zet`, as for #183 and its slices. Creating them publishes to GitHub, so it waits
   for the owner's word.
