# Bounded digest/resume pilot package (issue #90)

This is the deployment and pilot package for the last qualification slice of [PRD #64](https://github.com/liudesheng9/zet/issues/64). [#89](digest-release-qualification.md) qualified both frozen variants and the isolated performance/correctness release on a consistent archive copy. This slice installs that release, verifies the installed identity and the basic scratch boundaries, chooses the bounded source with its persisted identity, and hands the one authorized real pilot/resume run to the user's Claude Code session. The Stiglitz backlog recovery stays with [#91](https://github.com/liudesheng9/zet/issues/91).

## 1. What this slice verifies, and what remains

| Acceptance criterion | Evidence here | State |
|---|---|---|
| 1. Service/archive/run activity, authorization, consistent backup, no digest stopped implicitly | §2: activity record, expired run marker, no ledger, `VACUUM INTO` backup with `integrity_check` `ok` | done |
| 2. Matching binaries/integrations/frozen variants installed, installed identity, scratch read/batch smoke tests, rollback evidence | §3, §4, §5: four artifact hashes, service identity, skill parity, three `verify` results, the new `install-smoke` suite, rollback copies | done |
| 3. Bounded source with a formal model, a shared premise and declared historical/empirical work, with exact persisted source/versions | §6: source SHA-256, the prepared run contract, 3 groups, 3 Event candidates, `base_version`/`variant_version` | done |
| 4. Actual provider results, process lifetimes, receipt recovery, explanation/source coverage, owned cleanup through completion or one resume | §7: the run is the user's authorized Claude Code run; the exact invocation, injection and resume are fixed here, and every ledger command of the sequence is rehearsed on a consistent copy (§7.5) | **handed over** |
| 5. A completed neighbour's substantive build fingerprints/receipts unchanged, every partial outcome named | §7.4: the before/after ledger comparison; §7.5 already shows it holds across a real resume | **handed over** |
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

## 3. Installed identity

Built with `--release --locked` into the protocol's separate target directories, then installed. `zt`, `zt-boot` and `zt-check` are byte-identical to the artifacts of the pushed source at `7a7d04b` (no `src/` or `tools/zt-check/` change since), and `zt-digest` was rebuilt in this slice from the same source with the JSON-argument fix of §8.2, so the four installed files are the build of the current tree.

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
D:\zt-target\rollback\zt-digest-pre-issue90-20261018-163920.exe (the first install) and D:\zt-target\rollback\zt-digest-pre-issue90-inline-args-20261018-171720.exe (before the §8.2 rebuild)
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

The source is [`digest-pilot-short-source.zh.md`](digest-pilot-short-source.zh.md): 3,575 characters of Chinese prose in eight headings and a reference list, holding one formal model (a principal chooses a specific investment `x` under a convex cost and concave return, with renegotiation), one shared premise (「有限描述假设」, named as the premise both models depend on), one dated historical sentence (1937, Coase), a second dated one (Hart 1995) and one declared empirical result (a 12% investment increase over 1980–1990). It exercises the economics variant's formal model, shared premise, Event and Finding layers without a book-length run.

The source is calibrated for the **frozen workflow's own prepare command**, which passes `-MaxChars` and `-MaxUnits` but not `-MinUnitChars`: the tool's default minimum is 1000 characters per unit, so every one of the source's three units must exceed it. §8.3 records why that matters. The preparation below is exactly the workflow's form, run once with `-Apply` on a **consistent copy** of the live archive (never the live archive):

```powershell
zt-digest prepare -Variant econ -Source E:\zet\docs\digest-pilot-short-source.zh.md `
  -Out <work>\out -CitationKey Pilot2026 -BookTitle "Release qualification book" `
  -BibTeX "@book{Pilot2026,title={Release qualification book},author={Qualification, Ada},year={2026}}" `
  -RunOptions '{"terms":true,"models":true,"events":true,"findings":true,"hubs":true,"figures":false,"bridge":true}' -Apply
```

| Persisted fact | Value |
|---|---|
| Source | `E:\zet\docs\digest-pilot-short-source.zh.md`, SHA-256 `d0a27cd8b43252732b6532e08f9767f255c068efe0c550ed56fb2117a2922e94`, `file_id` 0 |
| Archive identity | `806009944a966daad513b2aacabddc5f` (the copy's resolved path is recorded too) |
| Contracts | base `3.4.0`, variant `4.4.0`, artifact schema 1 |
| Groups | 3, one unit each above the 1000-character minimum: `g01` 导论/Intro.1 (1,155 chars), `g02` 第一章/1.1/1.2 (1,181 chars), `g03` 第二章/2.1/2.2 (1,149 chars); anchors `Pilot2026/1..3` |
| Event candidates | 3 (`events.json`), including the 1937 Coase sentence, the Hart 1995 sentence and the data sentence |
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
    perManager: 1,
    events: true, findings: true, cardChecks: true,
    figures: false
  }
})
```

`perManager: 1` keeps one group's manager in charge at a time so the injection of §7.2 has a clear window; `events` and `findings` enable the two layers this source was chosen for; `figures: false` matches the source, which has no figures. The prepare defaults (`maxChars` 20000, `maxUnits` 3) already split the calibrated source into its three groups, so no sizing argument is needed.

These exact args were simulated through the workflow kernel in the stand-in runtime (`assemble.mjs simulate scripts/book-digest/workflow/zt-econ-digest.blueprint.json --sim … --mode full --args '<the object above>'`): `ok: true`, `halted: null`, every stage reached in order, `hubs-first` and `figures` skipped by the switches, 34 stand-in agent calls, peak 4. The same object with one undeclared key is refused (`ok: false`, exit 1). The stand-in runtime executes no shell command and starts no model, so this proves the handover command is legal for the frozen 4.4.0 script before the real run spends anything.

Do not add `until: "digest"` to this run. The checkpoint is a legitimate stop for a cheap inspection pass, but a resume after it spends the run's single resume budget (`ledger resume` reports `resumed` whenever unresolved units remain), which the injected failure needs.

### 7.2 The injected recoverable failure

The failure is injected into the run's own ledger while the digest stage runs. `ledger fail` takes a fault document (a file or the JSON text the workflow modules pass) whose code, unit and input hash must match the claim, and its class must be the class that code belongs to (a mismatch is refused before anything changes). `check_failed` is a `content` fault, so it leaves the unit `failed` until the resume; a `retryable` fault would instead put the unit back into `retry`, where the run's own attempt budget retries it and the pilot would reach its resume with nothing to reset.

```powershell
# 1. The claim the run holds: its manager claimed the group, and the ledger shows the token.
zt-digest ledger status -Out D:\zt-digest\pilot-90 |
  ConvertFrom-Json | Select-Object -ExpandProperty units |
  Where-Object { $_.state -eq 'claimed' } |
  Select-Object id, layer, owner, token, input_hash, lease_until

# 2. The fault file: the code, the unit and that claim's own input hash.
@{ schema_version = 1; unit = 'digest/g002@1'; input_hash = '<input_hash>';
   code = 'check_failed'; cause = 'pilot injected failure: the group fails once so the run reaches its one resume' } |
  ConvertTo-Json | Set-Content -Encoding utf8 D:\zt-digest\pilot-90\pilot-fault.json

# 3. The one injected failure: `content` is the class of `check_failed`.
zt-digest ledger fail -Out D:\zt-digest\pilot-90 -Unit digest/g002@1 -Token <token> `
  -Class content -Fault D:\zt-digest\pilot-90\pilot-fault.json
```

The unit records one attempt and stays `failed`; the run keeps the other groups and the layers they release and completes, and its report names the failed group and its blocked downstream work. Prefer a middle group (`g002`) so a completed neighbour exists on both sides. If the claim window is missed, the same injection works on any later claimed unit (a `models`, `explain` or `link` unit is equally recoverable and equally real); take the next claimed unit rather than a second resume.

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
| the injected failure and its one resume | `ledger report`, the `claimed`/`faulted`/`attempt_reset`/`resumed` ledger events, `resume.status` |
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

### 7.5 The ledger commands, rehearsed

[`scripts/book-digest/tests/pilot-rehearsal.ps1`](../scripts/book-digest/tests/pilot-rehearsal.ps1) rehearses every command of §7.1–§7.4 except the provider work, on a consistent copy of the live archive with the installed binaries. It is not part of the aggregate runner (it needs a copy of the archive, like `qualify-release.ps1`):

- it copies `-Archive` with `VACUUM INTO`, starts an isolated service and prepares the bounded source with the workflow's own command (§6, no `-MinUnitChars`, the switches as JSON text): 3 groups, the same source SHA-256 `d0a27cd8…`;
- it claims the run's next ready unit with the run's own `ledger next`, refuses a class the fault code does not permit, then injects the `check_failed`/`content` failure and asserts the unit stays `failed` with one attempt and 11 unresolved units;
- it runs the one supported `ledger resume`: invocation 2, `resume_count` 1, the failed unit reset, the three digest groups published as its downstream closure, the reset unit ready again, and the done `prepare/book@1` result, attempts and completion time unchanged;
- it asserts a second resume is refused as `resume_used` with nothing spent, that `ledger report` records invocation 2 and the resume and names 11 gaps instead of claiming completion, and that the copy still passes the full-parser Link check.

Result: **20 assertions, 0 failed** (`pilot-rehearsal-final.log`); the RED run with `-Digest` pointed at `zt.exe` fails at prepare and names the checks that did not run (`pilot-rehearsal-red.log`). Three defects in this document's earlier drafts were found this way and are fixed: `ledger fail` takes a fault document rather than a message; its class must be the code's own class, and `provider_transient`/`retryable` is retried inside the attempt budget so it would not reach the resume; and the source must be sized for the workflow's own prepare defaults (§8.3).

### 7.6 After the pilot

The pilot's Literature tree stays in the agent Workspace as evidence. To remove it afterwards, delete that tree with the normal confirmed delete (the GUI tree menu, or the shell `del` on `Pilot2026`), then run `zt status --check-links`. Keeping it is harmless: it is a normal Literature tree named `Pilot2026`.

## 8. Regression evidence at the installed release

| Check | Command | Result | Log |
|---|---|---|---|
| Root, all targets, locked | `CARGO_TARGET_DIR=D:\zt-target cargo test --all-targets --locked -- --test-threads=4` | **376 passed, 0 failed, 1 intentionally ignored** (the manual real-clipboard check) | `D:\zt-target\qualification\issue90\root-tests.log` |
| Checker, release, locked | `CARGO_TARGET_DIR=D:\zt-check-target cargo test --release --locked` | **10 passed, 0 failed** | `issue90\check-tests.log` |
| Digest, release, locked | `CARGO_TARGET_DIR=D:\zt-digest-target cargo test --release --locked` | **132 passed, 0 failed** | `issue90\digest-tests.log` |
| Aggregate public runner, throttle 3 | `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 3` | **63 suites, 0 failed, 311 s** (the 62 existing suites passed first, 302 s; the new installation smoke suite is the 63rd and ran inside this runner) | `issue90\public-summary.log`, `issue90\public2-summary.log`, `issue90\public4-summary.log` + the matching folders |
| Installation smoke | `pwsh -NoProfile -File scripts\book-digest\tests\install-smoke.tests.ps1` | **9 assertions, 0 failed**; the RED run exits 1 | `install-smoke-green.log`, `install-smoke-red.log` |
| Inline JSON arguments (§8.2) | `run-all.ps1 -Only ledger-preparation,ledger-claims` | **20 + 45 assertions, 0 failed**; both suites fail before the §8.2 fix | `issue90\red\`, `issue90\green\` |
| Pilot ledger rehearsal | `pwsh -NoProfile -File scripts\book-digest\tests\pilot-rehearsal.ps1 -Work D:\zt-target\qualification\issue90\rehearsal` | **20 assertions, 0 failed** (the workflow's own prepare form, the claim, the refused class, the injected content failure, one resume with invocation 2 and `resume_count` 1 and its closure, the unchanged done neighbour, the refused second resume, the report and Link parity); the RED run with a wrong digest binary exits 1 | `pilot-rehearsal-final.log`, `pilot-rehearsal-red.log` |
esume_count 1 and its closure, unchanged done neighbour, refused second resume, report, Link parity); the RED run with a wrong digest binary exits 1 | `pilot-rehearsal-committed.log`, `pilot-rehearsal-red.log` |
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

The fix is in `qualify-release.ps1`: the trial now removes any stale lock file, starts the holder, and polls for `zt.edit.lock` to exist (at most 15 s) before the measured write, with its own assertion naming how long the holder took to acquire it (one per variant, which is why the repeat reports 143 checks rather than 141). The holder's 6.5 s hold starts when it creates the file, so detecting the file leaves the hold at nearly its full length. A probe confirmed that a `FileShare.None` lock file is visible to `Test-Path` and refuses an open while it is held. The repeat then passed all 143 checks (§9); the failure and the fix are both retained.

### 8.2 A second defect: the workflow passes JSON text where the tool read only a file

Rehearsing the pilot's own prepare command (§7.5) found that the frozen workflow cannot pass its declared layer switches at all. `zt-digest-prepare.js` builds `-RunOptions '{"terms":false,…}'`, and `zt-digest-lib.js`'s `failUnit` builds `-Fault '<document>'`, but both tool arguments were read only as a **file path** (`canon::read_json`, `util::read_json`), so every such command failed with `Cannot find path '{"terms":…}' because it does not exist.` No suite caught it, because the framework harness runs a stand-in runtime that executes no shell command: the defect is exactly at the seam the harness cannot see.

The fix is test-first. `ledger-preparation.tests.ps1` now prepares a book with `-RunOptions` as JSON text and asserts the switches are frozen into the run contract (RED: `Cannot find path`), and `ledger-claims.tests.ps1` records a failure whose `-Fault` is text and refuses a stale token with text (RED: not `stale_claim`). `canon::read_json_arg` then accepts either form — text recognized by its opening brace or bracket, anything else read as a file, so a missing path keeps the original error and the 64 KiB fault bound applies to both forms — and `prepare` and `ledger fail` both use it. GREEN: 20 + 45 assertions, 133 digest unit tests (the new `canon` test covers file, text, array, missing path and malformed text). `zt-digest.exe` was rebuilt and installed (`CB840CCE…`), with the previous binary kept for rollback. The frozen workflows needed no change, so their qualified build stays the installed one.

### 8.3 A third defect: the bounded source did not fit the workflow's own prepare defaults

The same rehearsal showed the pilot source producing **one** group, not three. `prepare` merges the parsed units until each holds at least `-MinUnitChars`, whose default is 1000; the frozen prepare stage passes `-MaxChars` and `-MaxUnits` but never `-MinUnitChars`, so the original 1,074-character source was one unit and one group. A one-group pilot cannot show a completed neighbour group and, with `perManager: 1`, a failed single group would leave the digest stage with no done group at all — the stage halts and the run never reaches the layers the pilot exists to exercise. The earlier #89 package had prepared the source by hand with `-MinUnitChars 200`, a flag the workflow cannot pass.

The source is now calibrated for the workflow's defaults: 3,575 characters in eight headings, three units of 1,155, 1,181 and 1,149 characters. Under the workflow's exact prepare command it yields three groups (`g01` 导论/Intro.1, `g02` 第一章/1.1/1.2, `g03` 第二章/2.1/2.2 — the formal model and the empirical result, the shared premise, and the dated history respectively), and the rehearsal's own prepare assertion now proves that. The source's SHA-256 changed to `d0a27cd8…`, so §6 and the rehearsal record the new identity, and the T32 setup was repeated on the new source (§9).
## 9. T32: the isolated timing suite, repeated at the installed release

Method (R347–R348, R355, T32): one consistent copy of the representative archive per variant (`zt.sqlite3`, 10,895 Cards, 56,994 Link occurrences, revision 0), an isolated service per copy (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`), one real `prepare -Variant … -Apply` per variant (econ 562.2 ms, math 563.0 ms, 3 groups each, source `d0a27cd8…`), one warm-up per command, ten measured invocations (untraced), nearest-rank median and p95, and a traced warm-up for the call/address/process evidence. No digest workflow, provider call or live Card write ran; the live archive was only read to make the copies. The machine was shared during this repeat (other sessions' processes were running), so these are observed timings, not a guaranteed production latency.

| Variant | Command | Median ms | p95 ms | Min ms | Max ms | Target ms | MCP calls | MCP processes | Addresses read | Output bytes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| econ | `snapshot-topics` | 704.1 | 756.4 | 688.5 | 756.4 | 1,000 | 1 | 1 | 0 | 1,193,359 |
| econ | Card `plan` | 1,408.9 | 1,461.7 | 1,353.1 | 1,461.7 | 10,000 | 5 | 1 | 4 | 468 |
| econ | Card `check` | 1,437.7 | 1,486.3 | 1,382.4 | 1,486.3 | 10,000 | 5 | 1 | 4 | 59 |
| econ | Card `report` | 1,429.3 | 1,531.1 | 1,365.0 | 1,531.1 | 10,000 | 5 | 1 | 4 | 1,586 |
| econ | `prepare-link` | 5,172.4 | 5,347.6 | 4,956.4 | 5,347.6 | 20,000 | 56 | 1 | 10,899 | 237 |
| math | `snapshot-topics` | 701.9 | 759.0 | 677.3 | 759.0 | 1,000 | 1 | 1 | 0 | 1,193,359 |
| math | Card `plan` | 1,440.0 | 1,532.2 | 1,396.9 | 1,532.2 | 10,000 | 5 | 1 | 4 | 468 |
| math | Card `check` | 1,441.0 | 1,520.4 | 1,354.3 | 1,520.4 | 10,000 | 5 | 1 | 4 | 59 |
| math | Card `report` | 1,381.3 | 1,433.9 | 1,344.8 | 1,433.9 | 10,000 | 5 | 1 | 4 | 1,586 |
| math | `prepare-link` | 5,080.5 | 5,368.3 | 4,986.7 | 5,368.3 | 20,000 | 56 | 1 | 10,899 | 237 |

All ten command/variant combinations exit 0 on every measured run and meet their named target with both median and p95. `snapshot-topics` is one `export_titles` call and no Card scan; every Card step is one MCP process with five calls and four addressed Cards; `prepare-link` is one title export plus bounded text batches. The read-only phases advance no revision: `link_check.after_tools.archive_revision` equals `link_check.after_prepare.archive_revision` (1).

| Check | econ | math |
|---|---|---|
| Model apply (R352): 66 parts, 69 accepted addresses, ten applies | median 446.8 ms, p95 469.0 ms | median 462.5 ms, p95 727.7 ms |
| Chunk shapes, part map | `64+5` on all ten, 66 parts under `286/1` | `64+5` on all ten, 66 parts |
| Revisions per apply / replay revisions / lock lines | 2 / 0 / none | 2 / 0 / none |
| Batch row counts | `indexed_cards 5`, `refreshed_cards 67` | same |
| Ordinary read (R353) median / p95 | 2.7 / 3.3 ms, 1,422 bytes | 3.4 / 4.0 ms, 1,422 bytes |
| Ordinary write median / p95 | 12.1 / 15.0 ms, `indexed_cards 1`, `refreshed_cards 1` | 11.8 / 15.5 ms, same row counts |
| Lock contention (R348) | waited 6,643.1 ms; `queue_wait_ms=6632 held_ms=7` | waited 6,642.2 ms; `queue_wait_ms=6633 held_ms=5` |
| Declared concurrency | 4 of 4 applies commit, 8 revisions, no lock line | same |
| Fixed local work (R355) | control 10.9 ms / big (+256 Cards) 7.9 ms; `indexed_cards 1`, `refreshed_cards 3`, `changed_reverse_sections 0` on both; payload hash `3ac5b904…` identical; 3 Cards | shared |

The shared host kept this repeat slower than the first one (for example `prepare-link` 5.17 s against 4.92 s) but every command still meets its target with both median and p95, and no call or process count changed. The fixed-local-work comparison adds 260 addresses (4 Topics and 256 unrelated Cards) and neither the parsed-row count nor the latency grows.

Parity: `zt status --check-links` returns `ok` with no fault before the copy is touched, after prepare (10,899 Cards, revision 1), after the tool measurements (revision 1), and at the end of each variant. So the derived Link index and every stored Reverse section equal the full parser at each phase.

The machine-readable evidence is [`digest-release-latency-reverify-2026-10-18.json`](digest-release-latency-reverify-2026-10-18.json) (143 checks, 0 failed).

## 10. Limits

- The pilot run of §7 is **not** executed by this slice. It needs the user's authorization for the maintenance window, the provider spend and the live Card writes, and it runs from the user's Claude Code session. The pilot's own numbers (usage, latency, model coverage) will be pilot numbers; the whole-run R354 hour/agent/token targets stay unmeasured and are not claimed here.
- `until: "digest"` is a real checkpoint but it consumes the single resume budget if the run is continued with `mode resume`; the pilot therefore runs without it (§7.1).
- The live archive's `zt.digest.run` marker is expired but still present. It records no active run and the daemon ignores it; it is left in place rather than deleted, because the pilot's own run overwrites it and nothing in the archive depends on it.
- `zt-digest` was rebuilt in this slice for the JSON-argument fix (§8.2), so the release identity of §3 is the current tree's build rather than the #89 artifact for that one binary. The previous file is retained for rollback, and the 143-check timing repeat and all 63 suites were re-run on the new binary.
- The bounded source is calibrated for the workflow's prepare defaults (`-MinUnitChars` 1000, which the frozen stage cannot pass). A change to those defaults or to the source's headings needs the §8.3 measurement again: prepare must still yield at least three groups, or the pilot loses its completed-neighbour group and the digest stage can halt on the one failure.
- All evidence is from Windows, as before; Unix process-group behaviour is still unexercised.
- The one intentionally ignored root test remains the manual real-clipboard check.