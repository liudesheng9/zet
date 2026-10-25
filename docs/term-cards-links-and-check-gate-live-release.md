# Term Cards, Term Links and the Card check: S31 live release (issue #173)

This is the live-release record and runbook for **S31 / #173**, the deployment and deliberate
policy-promotion gate of [PRD #142](https://github.com/liudesheng9/zet/issues/142). It covers
design section 19 (R0564–R0575) and acceptance criteria S31-AC01–S31-AC07.

Source binding: the [implementation design](term-cards-links-and-check-gate-implementation-design.md),
section 19 (SHA-256 `a27224263597460d0822d7859cd1a822d47e7376bb207686689843fc6d3309d4`).
The machine-readable run is
[`term-cards-links-and-check-gate-live-release-evidence.json`](term-cards-links-and-check-gate-live-release-evidence.json).

## 1. What is delivered, and what stays with the owner

**Delivered and verified.** The live release procedure `scripts/release-live.ps1` and its test
`scripts/book-digest/tests/release-live.tests.ps1` (25 checks, 0 failed); the coordinated release
set installed and verified against `docs/term-cards-links-and-check-gate-release-manifest.json`
(the manifest's `zt_digest` was still `0.28.0` after S30 shipped 0.29.0 — fixed here); a consistent,
validated backup of the live archive; a read-only advisory observation of every live Card; and the
deliberate promotion/rollback mechanics with their owner-decision guard.

**Not claimed.** The gate is still **advisory**. No enforcement promotion ran: S31-AC04 needs the
S30 copied-book review (#172, **deferred by the owner**; #172 remains open) and the owner's written
decision, and the proposed week is an observation window, not an automatic activation. The advisory
observation is a baseline, not a completed week; the real `term-cards`/`term-link` pilot has not run,
so the live archive still has no Term Card and the observation reports no `T*` rule. No historical
backfill (S32/#174), no unlock, no archive restore, no macro strip, no head deletion and no digest
rerun happened.

## 2. Deployed set (one compatible set, R0565)

| Item | Value |
|---|---|
| `zt` | 0.40.0 (unchanged by this slice) |
| `zt-check` | 0.14.0 (unchanged) |
| `zt-digest` | 0.29.0 (S30's build, embedded base 5.6.0 / variant 6.6.0) |
| Frozen workflows, project **and** global | `zt-econ-digest` 6.6.0, `zt-math-digest` 5.6.0, base 5.6.0 |
| Agent guidance | `zt`, `zettelkasten`, `zt-digest-protocol` match the repository copies in `.claude`, `.codex` and `.dsh` |
| Rule contract | rule 4, diagnostic schema 1, applicability 2; archive schema generation 2 |
| Retained frozen history | 6.5.5 / 5.5.5 kept in both registries |

The global variants were re-installed from the project blueprints:
`node "$HOME/.claude/skills/workflow-framework-protocol/tools/assemble.mjs" install scripts/book-digest/workflow/<name>.blueprint.json --sim scripts/book-digest/workflow/<name>.sim.json --scope global`.
`scripts/verify-release.ps1` now verifies the project registry, the installed binaries, the skills,
the running daemon's published contract and a scratch advisory/hard-invariant check. A copied
binary's version is never treated as proof of the running daemon: the run reads the daemon's
`zt status` version and the contract published on a live MCP read.

## 3. The live run (2026-10-25, `D:\zt`)

Evidence folder `D:\zt-target\release-173/`; committed summary in
[`…live-release-evidence.json`](term-cards-links-and-check-gate-live-release-evidence.json).

| Step | Result |
|---|---|
| `-Step preflight` | **34 checks, 0 failed**: installed set and manifest agree; both project and global frozen variants are the manifest builds; three skills match in all three hosts; the live daemon is up with 0 Sessions, 5 Locks `integrity ok`, revision 7921, 1952 Cards, stage `advisory`, policy hash `447f8a…`; the stale `zt.digest.run` marker of the deferred S30 run (age 3117 s > 300 s TTL) is reported but does not block |
| `-Step observe` | read-only `zt-check --cards` over **all 1952** live Cards in seven bounded chunks: **15 stored legacy diagnostics** — 14 advice, 1 block-class (inactive while the stage is advisory) — `R2` 4, `I4` 3, `R7` 2, `L2` 2, `R4`/`R5`/`R20`/`I2` 1 each; **no `T*` rule** (no Term Card exists yet); revision and Card count unchanged |
| `-Step backup` | stopped the service, copied `zt.sqlite3` (33 366 016 bytes, SHA-256 `c098cc0d…1816`) to `D:\zt-release-backups\20261025-s31`, validated it by starting the current binary on a scratch copy — same archive id `80600994…`, revision 7921, 1952 Cards, 5 Locks `integrity ok` — then restarted the live service with the same identity, revision and Card count |
| `-Step promote` / `rollback` | **not applied**: no owner decision file exists. The guard refuses without one (see §4) |

An active digest is a hard refusal: `<archive>/zt.digest.run` is written by every `zt-digest` run and
renewed for 300 s (R318), so a release cannot start while a compiler/pass still holds the archive.
An open Session is also a refusal; `zt down` clears the service record before the daemon closes the
database, so the backup waits for the recorded pid to exit before copying.

## 4. Policy promotion and rollback (R0567, S31-AC04, S31-AC06)

The stage is the archive decision `gate.stage` (`advisory` default, `enforce` deliberate). A
promotion or rollback is a **policy write only**: it stores metadata rows and advances the revision
once per changed row; no Card, Lock, receipt or allocation is touched, and no Card text is rewritten.

A decision file is required:

```json
{
  "schema_version": 1, "issue": 173, "approved": true,
  "action": "promote", "stage": "enforce",
  "owner": "<owner>", "evidence": ["S30 review", "observation window"],
  "overrides": { "gate.agent.R2": "advise" },
  "note": "promote type/format first; style and formal rules stay advisory"
}
```

* **Promote type/format first**: `action: promote`, `stage: enforce`, with `advise` overrides for the
  agent style/formal rules not yet promoted (the domain-specific rule ids are the owner's choice).
* **Promote agent style/formal later**: a second decision that resets those overrides
  (`"value": "default"`) while keeping `enforce`.
* **Rollback**: `action: rollback`, `stage: advisory`. The overrides may stay or reset; the mandatory
  invariants are not policy-configurable and stay active in both stages.

The test proves the boundary on a scratch archive: a quality fault is stored under advisory,
refused under enforce, and stored again after rollback; the **mandatory Term-link target invariant
refuses in both stages**; the Card-text digest is identical before and after both policy changes; and
a promotion without a decision file is refused. `zt config policy` still names the invariants after a
rollback. This is a **policy** rollback; a binary/schema downgrade is different (below).

## 5. Interrupted compiler/pass recovery (R0568–R0573)

The existing digest machinery already carries this contract; S31 adds the release boundary around
it. For an interrupted Term-card or Term-link pass:

1. Read the frozen plan and the committed receipts from the run's ledger.
2. Verify the stored effects and the archive/source identity before retrying.
3. Resume the **same accepted operation keys**; never apply compensating deletion blindly.
4. Report the committed chunks and the pending work; a chunk is atomic, the whole book is not.
5. Stop on a Lock, an Access denial or a semantic conflict (`card_locked`, workspace denial,
   changed source/list); a stale or moved input is replanned, not forced.
6. Restore a backup only through a **separately authorized, reviewed recovery** that accounts for
   every later write. The S31 backup is not a restore command.

A policy rollback needs no conversion. An old executable may not understand Term Cards or macros, so
a syntax/schema downgrade needs its own reviewed conversion or backup recovery; this runbook does not
strip macros, retype Cards to Idea, delete heads or restore an archive to make it convenient
(R0575, S31-AC07).

## 6. Verdict

| Criterion | Verdict |
|---|---|
| S31-AC01 schedule outside digests; keep binaries/variants/integrations together | met: preflight refuses an active digest marker and an open Session; the set is deployed and verified as one |
| S31-AC02 consistent backup, validated migration/postflight, running-daemon contract | met for this release: consistent copy validated on a scratch start; daemon version and contract verified, not a copied binary |
| S31-AC03 observe advisory results and legacy debt; the week is no automatic activation | partial/owner-gated: baseline recorded (15 diagnostics, no `T*`); the observation window and the owner's reading remain |
| S31-AC04 promote type/format then style/formal deliberately after compatibility evidence | **not met — owner-gated**: needs the S30 review (#172, deferred) and the owner's decision file; no timer can promote |
| S31-AC05 document measured latency, refusals/fixes, version drift, limitations | met: S29's measured gate numbers (154–304 µs median, 7 statements) and the qualified limit are cited; this record adds the live observation and drift (the manifest's `zt_digest` 0.28.0 → 0.29.0) |
| S31-AC06 policy-only rollback preserves hard checks without rewriting Cards; schema downgrade documented | met for policy: the test proves no Card rewrite, the mandatory invariants in both stages, and the rollback; the schema boundary is documented in §5 |
| S31-AC07 do not unlock/backfill/strip/delete/restore to simplify | met: none of those operations ran |
