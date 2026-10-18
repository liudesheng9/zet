# Bounded digest/resume pilot package (issue #90)

This is the deployment and pilot package for the last qualification slice of [PRD #64](https://github.com/liudesheng9/zet/issues/64). [#89](digest-release-qualification.md) qualified both frozen variants and the isolated performance/correctness release on a consistent archive copy. This slice installs that release, verifies the installed identity and the basic scratch boundaries, chooses the bounded source with its persisted identity, and hands the one authorized real pilot/resume run to the user's Claude Code session. The Stiglitz backlog recovery stays with [#91](https://github.com/liudesheng9/zet/issues/91).

## 1. What this slice verifies, and what remains

| Acceptance criterion | Evidence here | State |
|---|---|---|
| 1. Service/archive/run activity, authorization, consistent backup, no digest stopped implicitly | §2: activity record, expired run marker, no ledger, `VACUUM INTO` backup with `integrity_check` `ok` | done |
| 2. Matching binaries/integrations/frozen variants installed, installed identity, scratch read/batch smoke tests, rollback evidence | §3, §4, §5: four artifact hashes, service identity, skill parity, three `verify` results, the new `install-smoke` suite, rollback copies | done |
| 3. Bounded source with a formal model, a shared premise and declared historical/empirical work, with exact persisted source/versions | §6: source SHA-256, the prepared run contract, 3 groups, 3 Event candidates, `base_version`/`variant_version` | done |
| 4. Actual provider results, process lifetimes, receipt recovery, explanation/source coverage, owned cleanup through completion or one resume | §7: the run is the user's authorized Claude Code run; the exact invocation, injection and resume are fixed here | **handed over** |
| 5. A completed neighbour's substantive build fingerprints/receipts unchanged, every partial outcome named | §7.4: the before/after ledger comparison | **handed over** |
| 6. Actual pilot usage/latency separate from whole-book targets; no Bridge proposal applied without its exact selection | §7.4: `ledger report` usage/timing, `bridge` report with no apply | **handed over** |
| 7. Demonstration through the actual command/MCP/compiler/workflow boundary, with T20/T25/T27/T29/T32 | §8: the suites and their results at this release | done |

Criteria 4–6 need real provider spend and live Card writes, so they are reserved for the user's maintenance window (§7). Nothing in this slice starts a digest workflow, starts a provider run, or changes a live Card.

## 2. Maintenance window: activity, backup, authorization

Activity record on 2026-10-18 before the maintenance window:

| Fact | Observed value |
|---|---|
| Service | `zt status`: up, archive `D:\zt`, 10,895 Cards, 0 Sessions, revision 0 |
| Digest activity | no `zt-digest` or `zt-check` process; `zt-digest service-wait`: `state: up`, `stage: null`, `last_exit_abnormal: false` |
| Registered-run marker | `D:\zt\zt.digest.run` is **expired**: `renewed` 15:34:20 with `ttl_s: 300`, `run_id: unknown`, `out: C:\Users\chenx\AppData\Local\Temp\adopt-live-copy` (a leftover adoption folder, not a live run). The daemon ignores an expired marker (`fresh_active_run` → `None`). |
| Ledger | the legacy staging `D:\zt-digest\stiglitz2014learning` holds no `ledger.sqlite3`, so no ledger run is open |
| Archive | `zt.sqlite3` last written 2026-10-17 22:45:31, revision 0, `archive_id` `806009944a966daad513b2aacabddc5f`, `links_index_version` 1 |

**Backup (verified).** `node:sqlite` `VACUUM INTO` on the live archive, because the archive is in WAL mode and a file copy misses changes:

```
D:\zt\backup\zt.sqlite3.20261018-163907.pre-issue90.bak   33,366,016 bytes
PRAGMA integrity_check -> ok
10,895 Cards, 56,994 Link occurrences, archive_id 806009944a966daad513b2aacabddc5f, revision 0
```

The backup is smaller than the source file because `VACUUM` rewrites it without free pages; the row counts and identity match the live archive.

**Nothing was stopped implicitly.** No digest was active, so the service stop below interrupted no run. The user's authorization is required for the actual pilot in §7, because it spends provider budget and writes Cards; this document records the planning and the verified preparation only.

## 3. Installed identity (release of `7a7d04b`)

Built with `--release --locked` into the protocol's separate target directories, then installed. The four installed files are byte-identical to those artifacts; `cargo build` reports the tree up to date at `HEAD = 7a7d04b` with a clean working tree, so the artifacts are the build of that commit.

| Binary | Version | SHA-256 (installed = artifact) | Installed path |
|---|---|---|---|
| `zt.exe` | `zt 0.20.0` | `751727E77F27F68466018F7E0003773363FFBA49F3B0EBE4A9BAF2DE831624E9` | `C:\Users\chenx\.cargo\bin\zt.exe` |
| `zt-boot.exe` | — | `BCE8F8966EA41FB5900441A14F45A0A0F1C82144437CCC9869774A941E42EE89` | `C:\Users\chenx\.cargo\bin\zt-boot.exe` |
| `zt-check.exe` | — | `36D0FD6195C0867746B87F97A2C06971E5BFDFDF112EACAFC08AC13E44F30A2F` | `C:\Users\chenx\.cargo\bin\zt-check.exe` |
| `zt-digest.exe` | `zt-digest 0.1.0` | `EF3FDE47DA58D89082AB6D73C141DB0F23D353097BEBC790A9937B8D53A767C7` | `C:\Users\chenx\.cargo\bin\zt-digest.exe` |

**Rollback copies** (the same four files as they stood before the install), retained for the whole pilot:

```
D:\zt-target\rollback\zt-pre-issue90-20261018-163920.exe
D:\zt-target\rollback\zt-boot-pre-issue90-20261018-163920.exe
D:\zt-target\rollback\zt-check-pre-issue90-20261018-163920.exe
D:\zt-target\rollback\zt-digest-pre-issue90-20261018-163920.exe
```

**Install and restart.** `zt down` → the four artifacts copied over the installed files → `zt up`. `zt.exe` was in use by one idle `zt mcp` child and kept its identical bytes; the other three were replaced. The daemon then started from the installed path:

| Check | Result |
|---|---|
| Daemon | pid 78420, `zt __daemon "D:\zt"`, `version=0.20.0` |
| `zt version` / `zt-digest version` | `zt 0.20.0` / `zt-digest 0.1.0` |
| `zt status` | up, archive `D:\zt`, 10,895 Cards, 0 Sessions, revision 0, GUI `http://127.0.0.1:4717/` |
| `zt status --check-links` | `ok: true`, 10,895 Cards, 56,994 occurrences, revision 0, 0 faults |
| `zt-digest service-wait -Wait 5` | `state: up`, `stage: null`, `reconnected: true`, `last_exit_abnormal: false` |
| GUI | `GET /` → 200; a tokenless `GET /api/ping` → 403 |

The live archive was read only. Installing the release advanced no revision. Rollback: restore the four files from `D:\zt-target\rollback`, and if an archive migration had run, restore the pre-install `zt.sqlite3` copy too; an older binary must not write an upgraded archive (R082). After a rollback, `zt status --check-links` must report `ok` before the archive is trusted.

## 4. Integrations and frozen variants

`scripts\install-agent-integrations.ps1` reinstalled the skills and re-registered the MCP servers:

- `skills/zt`, `skills/zettelkasten` and `skills/zt-digest-protocol` are byte-identical in `~/.claude/skills` and `~/.codex/skills` (0 mismatches over both copies of every skill file) and linked into `~/.dsh/skills`.
- Claude Code's user-scope server `zt` → `C:\Users\chenx\.cargo\bin\zt.exe mcp`, with its tools allowed; Codex's global server `zt` registered the same way.
- `zt mcp` answered a handshake with its 27 tools.

The installed frozen workflows are unchanged by this slice (it changes one test file and one test harness; no blueprint or module changed), so `verify` is the correct release action:

| Workflow | Version | `assemble.mjs validate` | `simulate` | `verify --scope project` |
|---|---|---|---|---|
| base `zt-digest-base` | — | 0 errors, 0 warnings (all slots empty) | — | — |
| `zt-econ-digest` | 4.4.0 | 0 errors, 0 warnings | `ok: true`, `halted: null`, 23 stages through `finalize → cleanup → report`, 35 agent calls, peak 4, 34 labels | `ok`, no problems, no notes, no library drift |
| `zt-math-digest` | 3.4.0 | 0 errors, 0 warnings | `ok: true`, `halted: null`, 26 stages through `math-check → cleanup → report`, 46 agent calls, peak 2, 45 labels | `ok`, no problems, no notes, no library drift |
| `zt-issue-dev` | 1.1.0 | 0 errors, 0 warnings | `ok: true`, `halted: null`, 10 agent calls | `ok`, no problems, no notes, no library drift |

The stand-in runtime executes no shell command and starts no model; the frozen scripts are the source versions.

## 5. Installation smoke test

`scripts\book-digest\tests\install-smoke.tests.ps1` is the repeatable form of the release package's installation verification. On a scratch archive it runs only the installed set, with no fault seam:

| Step | Assertion |
|---|---|
| identity | `zt version` and `zt-digest version` answer with their versions |
| read boundary | one real `zt mcp` stdio session (handshake, then `tools/call get_cards` with an empty address list) returns `complete: true`, no Cards, no missing address, revision 0 |
| batch boundary | one three-call identified batch through `zt-digest call -OperationPrefix install-smoke/scratch/1/build` commits three addresses, and its last Card names an earlier result of the same batch |
| receipts | replaying the same accepted batch returns the identical addresses and publishes no new revision; the batch published exactly one |
| read-back | `zt mcp get_card` returns the authored body byte for byte, with `[[0/0]]` resolved |
| checker | `zt-check --cards 0/1\|a` reports `faults: 0` |
| index parity | `zt status --check-links` reports the batch's three Cards and no fault |

Results: **9 assertions, 0 failed** against the installed release (`install-smoke-green.log`). A RED run with `-Digest` pointed at `zt.exe` fails at the version and batch assertions and exits 1 (`install-smoke-red.log`), so the check is live. The suite declares `-Zt`, so `run-all.ps1 -Zt <build>` tests a candidate set.

## 6. The bounded source and its persisted identity

The source is [`digest-pilot-short-source.zh.md`](digest-pilot-short-source.zh.md): four sections (1,074 characters) and a reference list, holding one formal model (a principal chooses a specific investment `x` under a convex cost and concave return, with renegotiation), one shared premise (「有限描述假设」, named as the premise both models depend on), one dated historical sentence (1937, Coase) and one declared empirical result (a 12% investment increase over 1980–1990). It exercises the economics variant's formal model, shared premise, Event and Finding layers without a book-length run.

The source was prepared once with `-Apply` on a **consistent copy** of the live archive (never the live archive) to prove that it prepares and binds exactly:

```powershell
zt-digest prepare -Variant econ -Source E:\zet\docs\digest-pilot-short-source.zh.md `
  -Out <work>\out -CitationKey Pilot2026 -BookTitle "Release qualification book" `
  -BibTeX "@book{Pilot2026,title={Release qualification book},author={Qualification, Ada},year={2026}}" `
  -MinUnitChars 200 -MaxUnits 2 -NoFigures -Apply
```

| Persisted fact | Value |
|---|---|
| Source | `E:\zet\docs\digest-pilot-short-source.zh.md`, SHA-256 `c9b20dca0e99e217bbc7dad0f8561c2443e9d162e252bb4fcc3e618d1ec39e13`, `file_id` 0 |
| Archive identity | `806009944a966daad513b2aacabddc5f` (the copy's resolved path is recorded too) |
| Contracts | base `3.4.0`, variant `4.4.0`, artifact schema 1 |
| Groups | 3 (`g01` 导论/Intro.1, 289 chars; `g02` 第一章/1.1, 203 chars; `g03` 1.2/第二章/2.1/2.2, 582 chars), anchors `Pilot2026/1..3`, group manifest hash `54438270…` |
| Group and section hashes | each group file and its sections retained (`g01` `ee7d12a1…`, `g02` `fe597570…`, `g03` `3d6cfd41…`) |
| Event candidates | 3 (`events.json`), including the 1937 Coase sentence and the data sentence |
| Findings | the empirical result is in the `g03` text for the Finding layer; no Finding is pre-declared by prepare |
| Source-fault report | `source-faults.md`, 0 faults of every class |
| Enabled layers | `prepare, terms, digest, cards, cards-book, models, explain, link, link-check, notes, organize, skeleton, hubs, bridge, audit, cleanup, events, findings` (figures and the math layers excluded) |
| Settings | `max_attempts` 3, `dsh_run_cap` 1200 s |
| Ledger result | `prepare/book@1` done at invocation 1 with its Card fingerprints and four receipt keys; 11 units open with their dependency edges; `alternatives` and coverage faults empty |
| Resume | the ledger reports the runnable resume invocation for `Pilot2026` |

The copy stayed at revision 1 with a passing `zt status --check-links` after prepare. The live archive was not written.

## 7. The pilot run (the remaining authorized step)

The pilot is one real `zt-econ-digest` run with one injected recoverable failure and at most one resume. It writes a new Literature tree (`Pilot2026`) into the agent Workspace of `D:\zt` and spends DeepSeek budget, so it runs only in the user's authorized maintenance window, from the user's Claude Code session.

### 7.1 Pre-flight

```powershell
zt status                      # up, revision unchanged, no digest stage
zt status --check-links        # ok, 0 faults
zt-digest service-wait -Wait 5 # up, stage null, last_exit_abnormal false
# the backup of §2 is the rollback point; take a fresh one if the archive changed since
```

Use a **fresh** staging folder. `mode full` prepares the run, and `prepare` refuses an already initialized output directory, so the staging folder is used once; the only continuation is `mode resume`.

```js
Workflow({
  scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js",
  args: {
    out: "D:\\zt-digest\\pilot-90",
    mode: "full",
    source: "E:\\zet\\docs\\digest-pilot-short-source.zh.md",
    citationKey: "Pilot2026",
    bookTitle: "Release qualification book",
    bibtex: "@book{Pilot2026,title={Release qualification book},author={Qualification, Ada},year={2026}}",
    maxUnits: 2,
    perManager: 1,
    events: true, findings: true, cardChecks: true,
    figures: false
  }
})
```

`perManager: 1` keeps one group's manager in charge at a time so the injection of §7.2 has a clear window; `events` and `findings` enable the two layers this source was chosen for; `figures: false` matches the source, which has no figures.

These exact args were simulated through the workflow kernel in the stand-in runtime (`assemble.mjs simulate scripts/book-digest/workflow/zt-econ-digest.blueprint.json --sim … --mode full --args '<the object above>'`): `ok: true`, `halted: null`, every stage reached in order, `hubs-first` and `figures` skipped by the switches, 34 stand-in agent calls, peak 4. The same object with one undeclared key is refused (`ok: false`, exit 1). The stand-in runtime executes no shell command and starts no model, so this proves the handover command is legal for the frozen 4.4.0 script before the real run spends anything.

Do not add `until: "digest"` to this run. The checkpoint is a legitimate stop for a cheap inspection pass, but a resume after it spends the run's single resume budget (`ledger resume` reports `resumed` whenever unresolved units remain), which the injected failure needs.

### 7.2 The injected recoverable failure

While the digest stage runs, poll the run's ledger and fail one group's claimed unit once:

```powershell
zt-digest ledger status -Out D:\zt-digest\pilot-90 |
  ConvertFrom-Json | Select-Object -ExpandProperty units |
  Where-Object { $_.state -eq 'claimed' } |
  Select-Object id, layer, owner, token, lease_until
```

Then, with the claimed unit's real token:

```powershell
zt-digest ledger fail -Out D:\zt-digest\pilot-90 -Unit digest/g002@1 -Token <token> `
  -Class transient -Message "pilot injected failure"
```

The failure is class `transient`, so the run records the attempt, keeps the group unresolved, and continues: the other groups and the layers they release still complete. Prefer a middle group (`g002`) so a completed neighbour exists on both sides. If the claim window is missed, the same injection works on any later claimed unit (a `models`, `explain` or `link` unit is equally recoverable and equally real); take the next claimed unit rather than a second resume.

### 7.3 One resume

After the first run settles (its report names the failed group and its blocked downstream work):

```js
Workflow({
  scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js",
  args: { mode: "resume", out: "D:\\zt-digest\\pilot-90" }
})
```

Expected: the prepare stage runs the one supported `ledger resume` — invocation 2, `resume_count` 1, only the unresolved units reset and their downstream closure drained, the done neighbours untouched. A second `mode: resume` must be refused as `resume_used`.

### 7.4 Evidence to collect

| Check | Command / file |
|---|---|
| every prepared group registered and either verified or named with its evidence | `zt-digest ledger status -Out D:\zt-digest\pilot-90 -Verify` |
| the Event and Finding candidates each have a disposition | `events.json`, `findings.json`, the `events-finish`/`findings-finish` results |
| the formal model's parts, closure and explanation obligations | `zt mcp` `get_model` on the Model Card, the `explain-finish` result |
| the shared premise is one Card linked by both models (where the source yields two) | `get_model` neighbours, `zt status --check-links` |
| the injected failure and its one resume | `ledger report`, the `attempt_reset`/`retry` events, `resume.status` |
| **nothing done was rebuilt** | run `zt-digest ledger status -Out <out> -Verify` before the resume and after it, then compare the done units' `result` and `completed_at` |
| coverage, placeholders and usage | `zt-digest ledger report -Out <out>` (usage per run, stage timing, gaps, placeholders) |
| latency separate from whole-book targets | the same `run-report.json` timing block: wall/queue/tool seconds and the critical path; the pilot's numbers are pilot numbers, never the whole-book R354 targets |
| no Bridge proposal applied without its exact selection | `bridge/report.md` and `bridge/proposals.json`; the run's `bridge` result must record report-only, and no `bridgeApply`/`bridgeApplyEarlier` selection exists |
| owned cleanup | the `cleanup-finish` result and `cleanup.resources`; no run-owned DeepSeek conversation or workspace left |
| the archive is still index-clean | `zt status --check-links` |

The before/after comparison of the done units, with the two status dumps saved as `before.json` and `after.json`:

```powershell
$facts = {
  param($path)
  (Get-Content $path -Raw | ConvertFrom-Json).units |
    Where-Object { $_.state -eq 'done' } |
    ForEach-Object { [pscustomobject]@{ unit = $_.id; attempts = $_.attempts; completed_at = $_.completed_at
                                        result = ($_.result | ConvertTo-Json -Depth 20 -Compress) } }
}
$before = & $facts before.json
$after = & $facts after.json
Compare-Object -ReferenceObject $before -DifferenceObject $after -Property unit, attempts, completed_at, result | Format-Table
```

An empty comparison is the AC5 result: every completed unit kept its result and completion time. Any row is a defect to report with the unit name.

### 7.5 After the pilot

The pilot's Literature tree stays in the agent Workspace as evidence. To remove it afterwards, delete that tree with the normal confirmed delete (the GUI tree menu, or the shell `del` on `Pilot2026`), then run `zt status --check-links`. Keeping it is harmless: it is a normal Literature tree named `Pilot2026`.

## 8. Regression evidence at the installed release

| Check | Command | Result | Log |
|---|---|---|---|
| Root, all targets, locked | `CARGO_TARGET_DIR=D:\zt-target cargo test --all-targets --locked -- --test-threads=4` | **376 passed, 0 failed, 1 intentionally ignored** (the manual real-clipboard check) | `D:\zt-target\qualification\issue90\root-tests.log` |
| Checker, release, locked | `CARGO_TARGET_DIR=D:\zt-check-target cargo test --release --locked` | **10 passed, 0 failed** | `issue90\check-tests.log` |
| Digest, release, locked | `CARGO_TARGET_DIR=D:\zt-digest-target cargo test --release --locked` | **132 passed, 0 failed** | `issue90\digest-tests.log` |
| Aggregate public runner, throttle 3 | `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 3` | **63 suites, 0 failed, 301 s** (the 62 existing suites passed first, 302 s; the new installation smoke suite is the 63rd and ran inside this runner) | `issue90\public-summary.log`, `issue90\public2-summary.log` + `issue90\public\`, `issue90\public2\` |
| Installation smoke | `pwsh -NoProfile -File scripts\book-digest\tests\install-smoke.tests.ps1` | **9 assertions, 0 failed**; the RED run exits 1 | `install-smoke-green.log`, `install-smoke-red.log` |
| Frozen framework harness | inside the runner (`framework-harness.mjs`) | pass, 172 checks | `public\framework-harness.mjs.log` |
| Bridge workflow harness | inside the runner (`bridge-workflow-harness.mjs`) | pass, 2 checks | `public\bridge-workflow-harness.mjs.log` |
| Workflow tools (no workflow run) | `node scripts/issue-dev/tests/issue-relay.tests.mjs` | **16 scenarios passed** | `workflow-tools.log` |
| Framework protocol own tests | `node …\workflow-framework-protocol\tests\run-tests.mjs` | **28 passed, 0 failed** | `workflow-tools.log` |
| Developer wiki | `node ~/.agents/skills/project-wiki/scripts/wiki.mjs check` | the wiki is current | — |

Named scenarios of this issue, all exercised through the public command/MCP/compiler boundary by the suites above:

| Scenario | Where |
|---|---|
| **T20** one resume starts with failures in multiple layers, drains all reachable work, retains history, preserves done builds, reports persistent failures without suggesting another automatic resume | `ledger-resume.tests.ps1` (43), `ledger-claims.tests.ps1` (43), `ledger-owner-recovery.tests.ps1` (31), `model-resume.tests.ps1` (20) |
| **T25** frozen simulations for economics and mathematics: ready Event/Finding continuations, models not queued behind block checks, Figures→Bridge parallel with Hubs, partial outputs, every join and finite final-fix path | `framework-harness.mjs` (172 checks) + `assemble.mjs simulate` for both variants (§4) |
| **T27** provider hangs past cap, cancellation returns late, service disappears and restarts, MCP never returns, child refuses clean shutdown; bounded recovery and owned-process cleanup; no concurrent replacement writer | `mcp-lifecycle.tests.ps1` (27), `service-health.tests.ps1` (53), `cleanup-resources.tests.ps1` (45) |
| **T29** cleanup after success/failure, lost creation response, already-deleted conversation, active conversation, unknown workspace member, network failure | `cleanup-resources.tests.ps1` (45) |
| **T32** the isolated timing suite, correctness parity and retained latency/call/row-count evidence | §9: `qualify-release.ps1` |

### 8.1 One defect found and fixed by this gate

The first repeat of the isolated timing suite failed 2 of 141 checks, both in the math variant's lock-contention trial: the measured write waited 11.8 ms instead of more than 5 s, and the daemon recorded no `edit wait done` line. The write had not contended at all. The cause is a race in the harness, not in the product: the trial started a PowerShell child that holds `zt.edit.lock` for 6.5 s and then slept a fixed 600 ms before measuring, but on a loaded host that child's startup can take longer than 600 ms, so the measured write took the lock before the holder did. The econ variant's identical trial passed in the same run.

The fix is in `qualify-release.ps1`: the trial now removes any stale lock file, starts the holder, and polls for `zt.edit.lock` to exist (at most 15 s) before the measured write, with its own assertion naming how long the holder took to acquire it. The holder's 6.5 s hold starts when it creates the file, so detecting the file leaves the hold at nearly its full length. A probe confirmed that a `FileShare.None` lock file is visible to `Test-Path` and refuses an open while it is held. The repeat then passed all 141 checks (§9); the failure and the fix are both retained.

## 9. T32: the isolated timing suite, repeated at the installed release

Method (R347–R348, R355, T32): one consistent copy of the representative archive per variant (`zt.sqlite3`, 10,895 Cards, 56,994 Link occurrences, revision 0), an isolated service per copy (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`), one real `prepare -Variant … -Apply` per variant (econ 520.3 ms, math 526.6 ms, 3 groups each), one warm-up per command, ten measured invocations (untraced), nearest-rank median and p95, and a traced warm-up for the call/address/process evidence. No digest workflow, provider call or live Card write ran; the live archive was only read to make the copies. The machine was shared during this repeat (other sessions' processes were running), so these are observed timings, not a guaranteed production latency.

| Variant | Command | Median ms | p95 ms | Min ms | Max ms | Target ms | MCP calls | MCP processes | Addresses read | Output bytes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| econ | `snapshot-topics` | 679.4 | 709.7 | 669.2 | 709.7 | 1,000 | 1 | 1 | 0 | 1,193,347 |
| econ | Card `plan` | 1,355.6 | 1,376.8 | 1,322.7 | 1,376.8 | 10,000 | 5 | 1 | 4 | 468 |
| econ | Card `check` | 1,343.6 | 1,381.1 | 1,324.5 | 1,381.1 | 10,000 | 5 | 1 | 4 | 59 |
| econ | Card `report` | 1,371.5 | 1,440.2 | 1,340.6 | 1,440.2 | 10,000 | 5 | 1 | 4 | 1,586 |
| econ | `prepare-link` | 4,917.8 | 5,087.0 | 4,844.6 | 5,087.0 | 20,000 | 56 | 1 | 10,899 | 236 |
| math | `snapshot-topics` | 711.7 | 764.6 | 668.9 | 764.6 | 1,000 | 1 | 1 | 0 | 1,193,347 |
| math | Card `plan` | 1,401.1 | 1,519.9 | 1,363.8 | 1,519.9 | 10,000 | 5 | 1 | 4 | 468 |
| math | Card `check` | 1,422.4 | 1,471.9 | 1,386.3 | 1,471.9 | 10,000 | 5 | 1 | 4 | 59 |
| math | Card `report` | 1,421.8 | 1,518.9 | 1,358.9 | 1,518.9 | 10,000 | 5 | 1 | 4 | 1,586 |
| math | `prepare-link` | 5,047.3 | 5,264.7 | 4,930.8 | 5,264.7 | 20,000 | 56 | 1 | 10,899 | 236 |

All ten command/variant combinations exit 0 on every measured run and meet their named target with both median and p95. `snapshot-topics` is one `export_titles` call and no Card scan; every Card step is one MCP process with five calls and four addressed Cards; `prepare-link` is one title export plus bounded text batches. The read-only phases advance no revision: `link_check.after_tools.archive_revision` equals `link_check.after_prepare.archive_revision` (1).

| Check | econ | math |
|---|---|---|
| Model apply (R352): 66 parts, 69 accepted addresses, ten applies | median 444.6 ms, p95 488.2 ms | median 442.9 ms, p95 532.5 ms |
| Chunk shapes, part map | `64+5` on all ten, 66 parts under `286/1` | `64+5` on all ten, 66 parts |
| Revisions per apply / replay revisions / lock lines | 2 / 0 / none | 2 / 0 / none |
| Batch row counts | `indexed_cards 5`, `refreshed_cards 67` | same |
| Ordinary read (R353) median / p95 | 2.6 / 2.8 ms, 1,422 bytes | 2.7 / 3.5 ms, 1,422 bytes |
| Ordinary write median / p95 | 11.9 / 228.1 ms, `indexed_cards 1`, `refreshed_cards 1` | 12.3 / 15.6 ms, same row counts |
| Lock contention (R348) | waited 6,645.2 ms; `queue_wait_ms=6631 held_ms=10` | waited 6,646.3 ms; `queue_wait_ms=6632 held_ms=10` |
| Declared concurrency | 4 of 4 applies commit, 8 revisions, no lock line | same |
| Fixed local work (R355) | control 11.1 ms / big (+256 Cards) 10.7 ms; `indexed_cards 1`, `refreshed_cards 3`, `changed_reverse_sections 0` on both; payload hash `3ac5b904…` identical; 3 Cards, 4,772 / 4,766 bytes | shared |

The econ ordinary-write p95 of 228.1 ms is one retained outlier sample on the shared host (its median and every other sample are 11–14 ms); it is reported rather than dropped. The fixed-local-work comparison adds 260 addresses (4 Topics and 256 unrelated Cards) and neither the parsed-row count nor the latency grows; the 6-byte reply difference is the `archive_revision` envelope of the larger copy.

Parity: `zt status --check-links` returns `ok` with no fault before the copy is touched, after prepare (10,899 Cards, revision 1), after the tool measurements (revision 1), and at the end of each variant (11,921 Cards, 58,974 Links, revision 44). So the derived Link index and every stored Reverse section equal the full parser at each phase.

The machine-readable evidence is [`digest-release-latency-reverify-2026-10-18.json`](digest-release-latency-reverify-2026-10-18.json) (143 checks, 0 failed).

## 10. Limits

- The pilot run of §7 is **not** executed by this slice. It needs the user's authorization for the maintenance window, the provider spend and the live Card writes, and it runs from the user's Claude Code session. The pilot's own numbers (usage, latency, model coverage) will be pilot numbers; the whole-run R354 hour/agent/token targets stay unmeasured and are not claimed here.
- `until: "digest"` is a real checkpoint but it consumes the single resume budget if the run is continued with `mode resume`; the pilot therefore runs without it (§7.1).
- The live archive's `zt.digest.run` marker is expired but still present. It records no active run and the daemon ignores it; it is left in place rather than deleted, because the pilot's own run overwrites it and nothing in the archive depends on it.
- All evidence is from Windows, as before; Unix process-group behaviour is still unexercised.
- The one intentionally ignored root test remains the manual real-clipboard check.
