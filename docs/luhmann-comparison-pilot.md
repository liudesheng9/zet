# One-book pilot under the frozen digest practices: gate and runbook (S26, #117)

Status: the gate and its rehearsal evidence are built and verified; **no book has been run**, and the gate's decision is **PENDING-HUMAN**. This document is the runbook for the one explicitly authorized pilot and the review package its qualification produces. It is not permission to run a book, to rewrite an old Card, or to publish a register or a hub.

The gate is `zt-digest pilot` ([`tools/zt-digest/src/pilot.rs`](../tools/zt-digest/src/pilot.rs)), one read-only command with three steps that write only under `-Work`:

```text
zt-digest pilot -Step plan      -Staging <out> -Work <folder> -File <authorization.json> [-Project <dir>]
zt-digest pilot -Step preflight -Staging <out> -Work <folder> [-Project <dir>]
zt-digest pilot -Step qualify   -Staging <out> -Work <folder> [-RunReport <run-report.json>]
                                [-Audit <audit.json>] [-Cards <cards/report.json>] [-Sample 12]
```

`<out>` is the staging folder of one prepared run under the installed frozen variant (economics `zt-econ-digest` 5.0.0 or mathematics `zt-math-digest` 4.0.0). The public behavior is covered by [`scripts/book-digest/tests/pilot.tests.ps1`](../scripts/book-digest/tests/pilot.tests.ps1) (53 assertions).

## 1. What the plan names before launch (S26-AC01)

`plan` reads the prepared run's own frozen contract and the archive, and it **refuses to write a plan** when any named fact is missing or does not verify. The record is `pilot-plan.json` (machine companion) and `pilot-plan.md` (the readable package) under `-Work`.

| Fact | Where it comes from | Checked |
| --- | --- | --- |
| Source identity | the prepared contract: path, SHA-256, file id | the file still hashes to the recorded bytes |
| Book and Citation key | the prepared contract | the launch args name the same Citation key |
| Staging and preparation | the prepared contract | the canonical staging path is this `-Staging` folder; base/variant contract versions differ from the installed blueprints → refused |
| Groups | the prepared contract: number, anchor, chapter, group-file SHA-256 | the approved group list equals the prepared groups |
| Archive identity | one identity read through `zt mcp` | id, path and revision are recorded |
| Installed binaries | the authorization record names `zt`, `zt-check`, `zt-digest` | each file exists; its SHA-256, byte count and own `version` are recorded; the named `zt-digest` must be the binary running the gate |
| Workflow versions | the installed frozen registry `.claude/workflow-framework/zt-<variant>-digest/lock.json` | the lock's version equals the prepared contract's variant version; the installed script hashes to the lock's `sha256` |
| Caps | the prepared contract's settings (`max_attempts`, `dsh_run_cap`) and the launch args | `maxAttempts` and `dshRunCap` are named and equal the prepared settings |
| Baseline sample | the read-only audit named in `baseline.report` | the report and its `audit-addresses.txt` are hashed; the report's scope, archive id, revision and sample are recorded |
| Backup | `backup` in the authorization record | the file exists, hashes to the recorded SHA-256 and byte count, and its `integrity` is `ok` |
| Preserved builds | `preserved_builds` | every named build exists, hashes to its record and names its version |
| Comparison | `comparison.previous_run` | the earlier run's staging folder exists |
| Approved new-Card scope | `approved_scope.statement` | nonempty; `approved_scope.old_card_edits` must be empty |
| Protected scope | every old Card of the archive | each one's authored fingerprint is written to `pilot-protected.json`; the old scope's digest and the prepared placeholders' digest are recorded |

The prepared Literature root and group anchors count as this run's own placeholders: they are named in `approved_scope.prepared_cards` and are expected to be written by the run. Every *other* Card that exists before launch is protected.

A launch record is one JSON object. The commands that produce its evidence are the same commands the release uses: `VACUUM INTO` for the backup, `digest-audit.ps1`/`zt-digest audit` for the baseline, and `Get-FileHash` for every named file.

```json
{
  "schema_version": 1,
  "approved_scope": {
    "statement": "One bounded pilot book: its new Cards only, no old-Card edit.",
    "groups": [1, 2, 3],
    "old_card_edits": []
  },
  "backup":  { "path": "<archive>/zt.sqlite3.<stamp>.bak", "sha256": "<hash>", "bytes": 0, "integrity": "ok" },
  "baseline": {
    "report":    { "path": "<baseline>/audit.json", "sha256": "<hash>" },
    "addresses": { "path": "<baseline>/audit-addresses.txt", "sha256": "<hash>" }
  },
  "preserved_builds": [ { "path": "<previous zt.exe>", "sha256": "<hash>", "version": "zt <version>" } ],
  "comparison": { "previous_run": "<earlier staging folder>", "variant_version": "<earlier variant version>" },
  "binaries": [
    { "name": "zt",        "path": "<installed zt.exe>" },
    { "name": "zt-check",  "path": "<installed zt-check.exe>" },
    { "name": "zt-digest", "path": "<installed zt-digest.exe>" }
  ],
  "launch": {
    "mode": "full",
    "args": { "out": "<out>", "citationKey": "<key>", "maxAttempts": 3, "dshRunCap": 1200 }
  }
}
```

`plan` also refuses a record whose `approved_scope.groups` differ from the prepared groups, whose caps differ from the prepared settings, or that omits an installed binary. It prints the exact launch line in `identity.launch.invocation`.

## 2. What preflight refuses (S26-AC02)

`preflight` re-verifies the plan and the launch state, writes `pilot-preflight.json` with every check and its problems, and exits 1 when any check fails. It refuses:

| Condition | Why |
| --- | --- |
| the plan record no longer hashes to its `plan_id` | an edited plan is a different, unauthorized plan |
| the run is on invocation 2, has spent its one resume, or any content unit is no longer `registered`/`ready` | old defect evidence and a moved run are never a fresh launch |
| a claim with an unexpired lease is open | a live writer owns the run |
| the staging folder already holds `owned-g*.txt` | the run already wrote Cards |
| a registered `link/gNNN@1` unit lacks its group's `digest/gNNN@R` **success** prerequisite, or `link-check/book@1` covers fewer groups than the run prepared | the #92 dependency path and the current lease/ownership protections |
| an installed binary or the frozen script changed since the plan | the qualification would name another release |
| the backup or a preserved build changed | the rollback point is not the one that was authorized |
| the archive identity differs, or its revision moved since the plan | the authorized scope moved; write a new plan |
| the protected scope's digest changed | a protected Card changed |

## 3. What the qualification reports (S26-AC03 – S26-AC07)

`qualify` reads the run's own retained evidence and writes `pilot-qualification.json` and `pilot-qualification.md`. It refuses to run without the run's verified `run-report.json`, and it never substitutes a simulation.

| Criterion | Evidence in the record |
| --- | --- |
| S26-AC03 | `cards.new` (every Card the run's scope now holds, with its owning group, parent, earlier-filing flag and recorded review outcome), `cards.unreviewed` (each one is a named gap), `cards.link_completions_missing`, `cards.groups[].link_quality`, `scope.changed`/`scope.removed` over the protected scope |
| S26-AC04 | `measures`: the audit's own `P1` (own-source Topic dependence: value, denominator, per-100 rate, target, `under_target`, `unknown_source` counted apart), `P2` restatements (candidates, judged, resolved, pending), `P3_P5` cross-book filing, `P4` appended tails and Link jobs with the exact per-run usage, and the pilot book's Card-language, Link-layer (L1/L2), orphan and unkinded counts |
| S26-AC05 | `interruption`: the run report's resume facts, the ledger's own fault/reset events and their invocations, the recovered groups from their retained `start.json`, and whether the supported interruption was exercised; a run without it says so |
| S26-AC06 | `old_data.changed`/`removed` (named addresses), `old_data.proposals` (the concrete old-card changes the run would need, each with its kind and evidence) and `old_data.applied` (always empty: the gate applies nothing) |
| S26-AC07 | `run`: the run report's identity, counts, completeness (`settled`, `content_complete`, `quality_complete`, `cleanup_complete`, `complete`, `quality_reason`), gaps, usage and timing; `measures.missing` names every unread measure with its reason |
| S26-AC08 | `review.sample`: a deterministic representative set (first Card of every group, one Model Card, one anchor, then address order), the pending decision, and the exact question |

The gate's own checks make the record self-auditing: `scope.no_old_change`, `cards.reviewed`, `cards.owned`, `link.ownership`, `groups.done` and `measures.known`. When any of them fails, `verdict` is `partial` — a partial pilot stays partial, and a simulation is never provider acceptance.

## 4. The decision (S26-AC08)

The qualification always records `decision: pending-human`. The human reviews the representative Cards and chooses **accept**, **correct** or **defer** for each, with evidence, and accepts or corrects the measured Topic dependence. Nothing in the gate closes that decision, and no elapsed time is approval.

## 5. Rehearsal evidence

[`scripts/book-digest/tests/pilot.tests.ps1`](../scripts/book-digest/tests/pilot.tests.ps1) drives the real public command on a scratch archive with one prepared run of [`digest-pilot-short-source.zh.md`](digest-pilot-short-source.zh.md). It starts no provider run and no Workflow, and it does not touch the live archive.

It proves, with **53 assertions** and **0 failures**:

- a plan without its authorization record, with an old-Card edit, with a cap that differs from the prepared contract, with a group set that differs from the prepared run, or without every installed binary is refused, and the refusal names the gate;
- a complete plan records the book, the frozen variant version, the installed script hash, all three binaries with their SHA-256, the caps, the exact launch line, the verified backup and the protected scope; `decision` is `pending-human`;
- preflight passes on a run that has not started and reports the Link prerequisite of every group; it refuses a run that moved, an archive that moved, a claimed unit, and it keeps its facts when refused;
- qualify refuses without the run's report; after a real group cycle (canned draft) it reports the run's owned Cards, the Card filed beside earlier thought, the review gap, the missing Link completion, the unknown baseline and the unexercised interruption, and it names the new Cards the run did not write;
- a retained Card-cycle review and Link completion are read per Card and per group; a changed protected Card is named by address and becomes one concrete old-data proposal; the gate's `scope.no_old_change` check fails; `old_data.applied` stays empty; the decision stays `pending-human`.

The run's own downstream producers are covered by their own suites: the Card cycle by `card-compiler.tests.ps1`, the Link pass and its completions by `link-gaps.tests.ps1` and `link-finish.tests.ps1`, the run report by `ledger-report.tests.ps1`, and the audit's measures by `audit-baseline.tests.ps1`.

## 6. The run (the remaining authorized step)

The gate plans and qualifies a launch; it never launches one. The authorized operator runs, in this order:

1. **Prepare** one book under the installed frozen variant, exactly as the Workflow does (`prepare -Variant <variant> -Book <Work id|title> -Out <out> … -Apply`), and guard it with a consistent `VACUUM INTO` backup of the live archive (the archive is in WAL mode, so a file copy misses changes).
2. Run a **read-only baseline audit** on a consistent copy (`zt-digest audit -Out <baseline> -SkipModels`) and keep its `audit.json` and `audit-addresses.txt`; they are the comparison scope the qualification reports.
3. Write the **authorization record** (§1) with the backup, the baseline, the preserved builds, the three installed binaries and the launch caps.
4. **`pilot -Step plan`**, then **`pilot -Step preflight`**. A refused preflight names every failed check; correct the record and plan again rather than forcing a launch.
5. Launch the plan's own `identity.launch.invocation` through the Workflow tool. If the run meets the supported interruption, use the one `mode: "resume"`; `ledger report` records it.
6. Render **`zt-digest ledger report -Out <out>`** and run the **qualification audit** (`zt-digest audit -CitationKey <key> -Out <audit work> -Baseline <baseline> -ConceptReport <concept review.json> -RunReport <out>/run-report.json -Decisions <restate decisions.json>`).
7. **`pilot -Step qualify`** with `-RunReport`, `-Audit` and `-Cards`; read `pilot-qualification.md`, review the sample, and record the decision.

`pilot -Step qualify` never writes a Card, a register, a hub or an archive row. A re-run over the same staging folder re-reads the same evidence and produces the same record apart from its own timestamp.

## 7. Boundaries

- Agents read both Workspaces and write only agent Cards. Human Reply creation remains an Owner GUI action.
- Finished models retain statements, derivations, structure, parts, membership and build order. No feature here authorizes an old-Card correction, a live register rebuild, a Bridge apply or a historical migration.
- Existing-Card changes require a concrete reviewed report and an exact user-selected scope. New pilot authorization covers only the identified new work, and `old_data.proposals` are the input to that separate gate.
- P6 Link-count caps/symmetric suppression and P10 Literature shortening/footprint remain dropped. The Link-scope/lease defect of #92 is safety this gate checks, not a claim that a prompt fixes it.
- Source tests, frozen simulation, the installed release, the actual provider pilot, human usability and a live historical apply are different evidence scopes. This gate's tests prove the gate; only the authorized run and the human decision qualify the pilot.
