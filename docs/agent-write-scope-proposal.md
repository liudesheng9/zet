# Write scope in the Write gate

Status: proposal (2026-10-31). Round 1 of the design is decided; round 2 is open (see the end).
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

## Design sketch

```text
client (Claude, zt-digest call, zt.mjs, GUI)
   │ declares a scope once per connection (round 2 decides how)
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

## Open questions (round 2)

These hang off D1 and D2; the next round settles them.

1. **How a job gets its scope.** A launch flag on `zt mcp` that `zt-digest` passes and a job
   cannot reach (the helper loses its job-facing `-Open`), or a grant the stage records in the
   archive and hands to the job as a token.
2. **The default scope of a connection that declares none** (depends on 1).
3. **The refusal shape.** A typed `scope_refused` object, carried like `card_locked`.
4. **One allow rule.** Merge the single-call rule and the identified rule (`@@N`, `-Beneath`).
5. **The Topic title rule.** Refuse or advise, and where the similarity threshold lives.
6. **The base `append_card` uses.**
7. **Rollout order** (depends on all of the above).
