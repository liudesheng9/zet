# S28 installed-reader release: qualification and rollback (issue #121)

This is the release package for **S28 / #121**, the human release gate of [PRD #93](https://github.com/liudesheng9/zet/issues/93) (US042, design §15-phase-4/5/release). It records what was installed, what the installed service actually answers, which suites prove the code, what was retained for rollback, and what remains the human reader's own evidence.

**Delivered and verified here.** The release is built from `main`, installed through the existing lifecycle, and running: `zt 0.21.0`, live archive `D:\zt` at revision 0 (10 895 Cards, 211 Topics, 7 Literature trees, 56 994 Link occurrences, 0 broken links), GUI `http://127.0.0.1:4717/`. The three independent reader switches work on the live service; entry and plain search, the Ask proposal (matched, sparse and low-confidence/truncated), the agent Hub draft plan, the Reply refusals and the one distant Card were exercised through the installed routes; the MCP server reports 0.21.0 with its 29 tools and still cannot write a human Card; this 0.20.0 predecessor, a verified backup, the Spreads document and the frozen Workflow locks are retained for rollback; the agent integrations were reinstalled and every installed skill file now matches the repository.

**Not claimed here.** The fixed task-sheet run on these flows (S28-AC03), the Owner Save/Keep observation and the keyboard/attention inspection (S28-AC04) are the human reader's own task and are **deferred, not done** (`docs/luhmann-reader-task-sheet.md`, §7 below). The real provider pilot stays with [#117](https://github.com/liudesheng9/zet/issues/117), the existing-register publication with [#119](https://github.com/liudesheng9/zet/issues/119), and the one-book run with the user's own Workflow run. No number in this package is a task-sheet observation.

## 1. Release identity

Installed from `main` at `38c53e6`; the slice's three code commits are `d407268` (the three switches), `120baf9` (the fixed task sheet) and `38c53e6` (the page half), and this package is the commit that records their evidence. The release build is `D:\zt-target-121-release\release` and each installed file is that build byte for byte. Rebuilding that directory from the current source reproduces `zt.exe` with the same SHA-256, so the installed binary is this tree, not merely a later build of it:

| Binary | Installed path | SHA-256 | Version |
|---|---|---|---|
| `zt.exe` | `C:\Users\chenx\.cargo\bin\zt.exe` | `7D076E13307F542896CDCCCADA8EFFE73C5C525DCD5B77A77FDC01F92CD932CD` | `zt 0.21.0` |
| `zt-boot.exe` | `C:\Users\chenx\.cargo\bin\zt-boot.exe` | `38E652554AB5B414B57814B6D82BE028DAC27FCC5335800DDDDFCD2834F4FFAA` | (same package) |
| `zt-check.exe` | `C:\Users\chenx\.cargo\bin\zt-check.exe` | `F6641653CCD9AA7AEB4C65A7A31BC4028A98537F7A95D2BB31039062634600E2` | `0.1.0` (unchanged) |
| `zt-digest.exe` | `C:\Users\chenx\.cargo\bin\zt-digest.exe` | `C402625FFEC00F867ED1015833594067FBC0E503BD5E105F3E65D275F6426AA4` | `0.4.2` (unchanged) |

The package version moves from 0.20.0 to 0.21.0 in this slice so "the installed release" is distinguishable from the previous one; the four `zt` files say `0.21.0` (`zt version`, the daemon's `version=0.21.0`, and the MCP `serverInfo.version`).

Running service (`zt status`). The install's own start was pid 68912; that daemon ended without `zt down` when the machine's session closed, and the sign-in hook (`zt-boot`) brought the same release back as pid 45852 at the same version, archive and revision. `zt.log` records both, and `zt status` keeps the earlier end as `last_exit: abnormal … panicked unknown` precisely because there is no normal exit record — an OS session end, not a code panic, with revision 0 and every count unchanged before and after:

```text
state: up
pid: 45852
gui: http://127.0.0.1:4717/
archive_root: D:\zt
sqlite: D:\zt\zt.sqlite3
revision: 0
daemon: version=0.21.0 started=1792465058 last_heartbeat=unknown
last_exit: abnormal pid 68912 version 0.21.0 revision 0 started 1792464219 panicked unknown (no normal exit record; see zt.log)
cards: 10895
sessions: 0
autostart: on
```

`zt status --check-links`: `{"archive_revision":0,"cards":10895,"links":56994,"faults":[],"ok":true}`; archive identity `806009944a966daad513b2aacabddc5f`; overview fingerprint `d21217d4e6cd785cb5ea7539`. The pre-install and post-install values are identical (revision, counts, Links, identity, fingerprint), so the install changed no Card.

Frozen Workflows (installed, verified by `assemble.mjs verify --scope project`):

| Workflow | Version | Installed script SHA-256 | Verify |
|---|---|---|---|
| `zt-econ-digest` | 5.0.0 | `51572E82E8CF3D60729A2A03D42E63DF5B1C8768588507ECB1DB0E364AB03446` | `ok: true`, `problems: []`, `libraryDrift: []` |
| `zt-math-digest` | 4.0.0 | `9F83B3A399B50D3F02784795EA048E383BA1C9DDE099C8E0895CA754F2B85753` | same |
| `zt-issue-dev` | 1.3.0 | `D5BAC275241962679DCC8327C52DE16384C16B437C066943BCB726AA14A55074` | same |

No Workflow source changed in this slice, so nothing was reassembled or reinstalled: the installed variants are still the ones #115 installed on 2026-10-19 (5.0.0 economics, 4.0.0 mathematics).

## 2. Lifecycle and active-work inspection (S28-AC01)

Before anything was stopped or replaced:

- `zt status` showed the service up on `D:\zt` (10 895 Cards, 0 Sessions, autostart on) and `zt status --check-links` was already `ok` with 0 faults.
- The only run lease was `D:\zt\zt.digest.run`, written by the agent's own audit rehearsal: `pid` dead, `renewed` 724 s before the check, `ttl_s` 300, so it was an **expired** lease, not active work. The earlier `pilot-90` lease was likewise stale (age ≈ 26 h).
- No `zt.edit.lock` existed; `zt.sessions` was empty; no book digest or digest stage was running.
- The install followed the documented rule "stop the service before replacing the binary": `zt down`, replace `zt.exe`/`zt-boot.exe` by rename-then-copy (so the running copies are never overwritten), `zt up` (which is the command that runs `initialize_database` and the migrations; `zt up` while the old service is up would skip them).
- The previous installed `zt.exe` was exactly the 0.20.0 build from the last release; its replaceable copies are retained (§6).

The service log shows the orderly stop/start pair and the new daemon (`gui started on http://127.0.0.1:4717/`, `version=0.21.0`).

## 3. Suites (code and tool contracts)

All runs are on Windows from `E:\zet`, with the separate target directories the repository uses because the live service holds `target\debug`.

| Check | Command | Result |
|---|---|---|
| Root, all targets, locked | `CARGO_TARGET_DIR=D:\zt-target cargo test --all-targets --locked` | **527 passed, 0 failed, 1 ignored** (the manual real-clipboard smoke test). Logs: `D:\zt-target\qualification-121-root-tests-final.log`, re-run `…\issue121\reverify\root-tests.log` |
| Checker, locked | `CARGO_TARGET_DIR=D:\zt-check-target cargo test --manifest-path tools/zt-check/Cargo.toml --locked` | **20 passed, 0 failed** (`reverify\zt-check-tests.log`) |
| Digest tools, locked | `CARGO_TARGET_DIR=D:\zt-digest-target cargo test --manifest-path tools/zt-digest/Cargo.toml --locked` | **273 passed, 0 failed** (`reverify\zt-digest-tests.log`) |
| Aggregate public runner | `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 4 -Zt D:\zt-target-121-dbg\debug\zt.exe -Logs D:\zt-target\qualification\issue121\reverify` | **83 tests, 0 failed** in 580 s (`reverify\`; the earlier run is `public-final\`) |
| Frozen framework harness | inside the public runner (`framework-harness.mjs`) | pass |
| Framework protocol own tests | `node %USERPROFILE%\.claude\skills\workflow-framework-protocol\tests\run-tests.mjs` | **28 passed, 0 failed** |
| Workflow tools (issue relay) | `node scripts\issue-dev\tests\issue-relay.tests.mjs` | **23 scenarios passed** |
| Workflow assembly | `node …\assemble.mjs verify <name> --scope project --project E:\zet` | `ok: true`, `problems: []`, `libraryDrift: []` for all three frozen Workflows |
| Developer wiki | `node %USERPROFILE%\.agents\skills\project-wiki\scripts\wiki.mjs check` | **the wiki is current** (16 blocks, HEAD `38c53e6`) |

Everything above was run a second time from the same tree before this package was written, so the numbers are the current source's, not a remembered run: 527 + 20 + 273 + 23 tests and 83 public suites, all green; the three `assemble.mjs verify` answers show `ok: true` with `problems: []` and `libraryDrift: []` and the installed scripts still hash to `51572E82…` (econ 5.0.0), `9F83B3A3…` (math 4.0.0) and `D5BAC275…` (issue dev 1.3.0).

The slice's new coverage: `src/features.rs` (5 unit tests), `tests/reader_release.rs` (3 tests), and the page tests the GUI half added in `src/gui.rs` (4 tests, including the 4 s overview poll). The root run also re-runs every earlier suite, so the 515-test release of #120 stays green at 527 with the new tests.

The Workflow **tools** were tested; the digest Workflow itself was not run here (the user runs it with Claude Code).

## 4. The three reader switches (S28-AC07)

`zt config set gui_search|gui_ask|gui_reply on|off` writes one line in `config.toml`; the daemon reads it on every request, so a switch needs no restart. A disabled feature's own route answers `409 {"error":"the <name> feature is disabled in the GUI; run `zt config set <key> on`","disabled":"gui_search|gui_ask|gui_reply"}`, `/api/overview.features` reports the three states, and the page removes that feature's controls whole (the GUI half). Every other route keeps working.

Verified on the **installed** service, one switch at a time (evidence `D:\zt-target\qualification\issue121\live-switches.json`):

| Switch off | Its route | Other two features | Reader paths | Page | Revision |
|---|---|---|---|---|---|
| `gui_search` | `/api/search` → 409, `disabled: gui_search` | ask on, reply on | `/api/card` 200, `/api/spreads` 200 | 200 | 0 → 0 |
| `gui_ask` | `/api/ask` → 409, `disabled: gui_ask` | search on, reply on | `/api/card` 200, `/api/spreads` 200 | 200 | 0 → 0 |
| `gui_reply` | `/api/reply` → 409, `disabled: gui_reply` | search on, ask on | `/api/card` 200, `/api/spreads` 200 | 200 | 0 → 0 |

Each was turned back `on` and `overview.features` returned to `{"search":true,"ask":true,"reply":true}` with the same route answering again. The MCP server is not gated (the switches are the GUI's). `tests/reader_release.rs` proves the same behavior on a scratch service, including one switch at a time, all three off, unknown config lines surviving a write, an invalid value refused, and the choice surviving a restart.

Re-run on the running service after it came back from the sign-in hook (`D:\zt-target\qualification\issue121\reverify\live-reverify.json`): each switch was turned off and back on once more; every `409` named its own key, the other two routes and the page answered 200, the Card, the Spreads and the revision stayed as they were, and all three ended `on`.

## 5. Installed reader flows (S28-AC02, AC04)

Driven through the installed routes and `zt mcp` against the live archive; every read is recorded with the same revision before and after (evidence `live-reader-checks.json`, `live-mcp.json`):

| Flow | Installed answer |
|---|---|
| Entry search `消费者偏好` (`mode=entry`) | 21 rows; the first rows are `role: entry` (`rank_tier` 2); 2 entry rows; agent register `196/1` with 78 entries, 0 faults; `register_missing: false`; `folded_total: 0`; bounds reported |
| Plain search `限制资本流动` | 6 rows, first `270/3`; `type=model` narrows to 3 rows, every one a Model Card; `ws=human` returns only human Cards (4 rows for `sylla2018alexander`) |
| Ask `消费者偏好如何决定需求函数？` | 2 matched terms, 7 explained Cards, each with its reason; no write |
| Ask `量子引力如何统一四种基本力？` | `sparse: true`, 0 Cards, 0 terms — an honest empty answer |
| Ask `汇率制度与资本流动的关系是什么？` | `fallback.kind: bigram`, `confidence: low`, `truncated: true`; `counts` shows the walk hit its bounds (200 visited, 40 seeds with 28 dropped, `cards_truncated: true`, 293 Links); the question is kept whole in `spread.question`, and the bounds are reported |
| Hub draft of `270/3` + `stiglitz2014learning/149|a|a|2` | plan: agent Workspace, `level: field`, destination `196/1` (`how: side`), 2 Tree groups, `check.ok: true`; a short reason is refused as `bare` before Save |
| Reply to the human Card `0/1` | `{saved:false, reason:"target_not_agent"}` (200), the human Card's authored fingerprint unchanged |
| Distant slot of `270/3` | exactly one candidate: `mascolell1995microeconomic/98|a`, term `外部性`, `source: model_name`, no Bridge report; the focus Card's source is `stiglitz2014learning`; no Card, Link, Spread or revision changed |
| MCP (`zt mcp`) | `serverInfo.version` 0.21.0, 29 tools; entry search 21 rows; human search 3 rows; `edit_card` on human `0/1` is an error — "the human workspace is read-only for AI agents" — and the Card's fingerprint is unchanged |

`reads_wrote_nothing: true` in the evidence: revision 0 → 0, Cards 10 895 → 10 895, Links 56 994 → 56 994, 0 faults before and after.

Re-run against the running service at this package's commit (`reverify\live-reverify.json`, script `reverify-live.ps1`), with the same answers row for row: entry 21 rows (`role: entry`, tier 2, 2 entry rows, `folded_total: 0`, register `196/1`, 78 entries, 0 faults); plain 6 rows with the two filters; Ask 7 explained Cards, then `sparse: true` with 0 Cards, then `bigram`/`low`/`truncated` with `{visited: 200, seeds: 40, seeds_dropped: 28, cards_truncated: true, links: 293}` and the whole question echoed; hub draft `field` under `196/1` (`how: side`, 2 Trees, `check.ok: true`) and the short reason refused `bare`; Reply `target_not_agent` with the human Card's authored fingerprint unchanged; exactly one distant candidate `mascolell1995microeconomic/98|a` (term `外部性`, `source: model_name`, no report); MCP `0.21.0` with 29 tools and the refused human `edit_card`. The revision, Card count and Link count were identical before and after, and `/api/overview` still answered fingerprint `d21217d4e6cd785cb5ea7539`.

**+N recovery** has no instance in this archive: the register has no accepted `## 同述入口` entry and no two Idea Cards share a statement fingerprint, so `folded_total` is 0 and the search tray has no `+N` row to open at revision 0. The folding itself is proven on scratch archives (`tests/search_rank.rs`, the S24 package's matrix); this release package does not claim a live observation of it.

**Term ambiguity** likewise has no instance: the live agent register has 78 distinct accepted terms with 0 ambiguity faults, so no question triggers `ambiguous_terms` on this archive; the behavior is proven by the scratch suites (`tests/entry.rs`, `tests/ask_spread.rs`).

## 6. Rollback retention and runbook (S28-AC06, D381)

Retained before the install (all paths and hashes are re-checkable):

| Item | Location | Evidence |
|---|---|---|
| Previous installed `zt.exe` (0.20.0) | `D:\zt-target\qualification\issue121\rollback\20261019-214227\zt-0.20.0.exe`, also `C:\Users\chenx\.cargo\bin\zt.exe.old-20261019-214227` | SHA-256 `D350F0034CA2F577F44CE3EF84D7859BBE12F8C8241D5D06F695C590F2F89B04`, runs `zt 0.20.0` |
| Previous `zt-boot.exe` | same folder, `zt-boot-0.20.0.exe` | SHA-256 `7984EBBBBB0F7A9271822AE2E61B59A9ADC2C5A69676ACE28D756EC492CAF93D` |
| Installed Workflow versions and locks | `…\rollback\20261019-214227\workflows\<name>\lock.json` plus `.claude/workflow-framework/*/history/` | econ 5.0.0, math 4.0.0, issue-dev 1.3.0; every historical version is still in `history/` |
| Verified archive backup | `D:\zt\backup\zt.sqlite3.20261019-214215.pre-issue121.bak` | 33 366 016 bytes, SHA-256 `26a210347d02…`, `PRAGMA integrity_check` = `ok`, 10 895 Cards, archive identity `8060…`, same `gui_spreads` row; made after `zt down`, before any write (`backup-verified.json`) |
| Spreads document | `D:\zt-target\qualification\issue121\spreads-export-before-install.json` | `{current, spreads}` exactly as `/api/spreads` answered; at revision 0 it holds one empty Spread, so no whole `question` depends on it today. Export it again after the human run, before any binary rollback that drops the optional `question` field |
| Release build identity | `D:\zt-target-121-release\release\zt.exe`, `build-final.log`, installed hashes above | installed file = build file, byte for byte |

Rollback runbook (code rollback only; archive data is not touched):

```powershell
zt down
# keep the running 0.21.0 aside, then restore the retained 0.20.0 build
Copy-Item C:\Users\chenx\.cargo\bin\zt.exe C:\Users\chenx\.cargo\bin\zt.exe.0.21.0
Copy-Item D:\zt-target\qualification\issue121\rollback\20261019-214227\zt-0.20.0.exe C:\Users\chenx\.cargo\bin\zt.exe -Force
Copy-Item D:\zt-target\qualification\issue121\rollback\20261019-214227\zt-boot-0.20.0.exe C:\Users\chenx\.cargo\bin\zt-boot.exe -Force
zt up
zt version    # must print zt 0.20.0
zt status     # daemon: version=0.20.0; the Cards and Spreads stay readable
```

The 0.20.0 reader has no `/features.js`, no `/api/overview.features` and no `gui_*` config lines; it drops the optional Spread `question` field on its next Spread save, so preserve the exported Spread document first if that field matters (it does not at revision 0). Archive restore, if it is ever needed, is the ordinary "copy the verified backup over `zt.sqlite3` while the service is down", and it is a replacement of Cards, not an undo of them: the existing history promises **no automatic undo** for multi-step content transfers (`topic-home` and `topic-join` retain the inverse of each applied edit as *evidence*, not as a rollback command). The check that a dump or backup succeeded is its output (`integrity_check`, counts), never the fact that a command started.

**Older prepared runs stay pinned.** The retained pilot ledger `D:\zt-digest\pilot-90-defect-evidence\ledger.sqlite3` records its preparation contract as `{base_version: 3.4.0, variant_version: 4.4.0}` (2026-10-18), while the installed variants are 5.0.0/4.0.0: nothing reassembled or upgraded that run, and its receipts remain bound to the contract it was prepared under.

## 7. The fixed user task sheet (S28-AC03, AC04) — deferred to the human reader

**Status: not run.** The installed service, the routes and the backup are ready, but the task sheet and its two Owner writes (Keep one Spread; Save one Reply) are the human reader's own judgement and action. The main agent declined to fill the sheet on the reader's behalf, because "useful", "kept" and the Owner's own reply cannot be supplied by an agent without fabricating the evidence. Nothing has been written to the live archive for this, and no sheet numbers exist.

The fixed sheet, the four tasks and every field to record are [`docs/luhmann-reader-task-sheet.md`](luhmann-reader-task-sheet.md). The handoff:

1. Open `http://127.0.0.1:4717/` (installed 0.21.0, live `D:\zt`).
2. Do T1–T4 in order and fill `D:\zt-target\qualification\issue121\reader-task-sheet.json` in the documented `zt-task-sheet` schema (interval, seconds, `useful_address`, `entry_rank`, `kept_spread`, T4 offered/opened/laid, `human_replies`).
3. Record the PASS/FAIL observations beside the sheet (§"The checks beside the four tasks" of that document).
4. Turn the sheet into the measures:

```powershell
zt-digest audit -Out D:\zt-target\qualification\issue121\audit `
  -TaskSheet D:\zt-target\qualification\issue121\reader-task-sheet.json -SkipModels
```

`-SkipModels` avoids one `get_model` read per model head on the whole archive; P12/P13/P15 and P14 do not need model structures. The audit writes only its `-Out` folder (and an archive run lease); it writes no Card. On this archive it takes about six minutes. `audit.json` then reports `baseline.measures.P12_P13` (time to first useful Card and kept Spread, entry rank, removed Cards, human replies), `P15` (distant Cards offered/opened/laid) and `P14` (human Cards and human-to-agent Links) with the sheet's interval and validity.

Until that run, S28-AC02's browser observations, AC03's measurements and AC04's Owner-Save observations stay **unverified** and are named as such. The 527-test code evidence, the installed-route checks and the rollback retention above do not substitute for them.

## 8. Agent integrations (S28-AC08, D379)

The installed skill copies were stale: `skills/zt/SKILL.md` and `skills/zettelkasten/SKILL.md` in the repository had moved ahead of `~/.claude/skills` and `~/.codex/skills` (the `zt` guidance lacked the #118 "File as card" paragraph). `scripts/install-agent-integrations.ps1 -ZtPath C:\Users\chenx\.cargo\bin\zt.exe` was re-run after the install; it:

- replaced the three skills (`zt`, `zettelkasten`, `zt-digest-protocol`) in `.claude/skills` and `.codex/skills`, re-linked the three `~/.dsh/skills` junctions, and backed up `~/.claude.json` and `~/.codex/config.toml` as `*.zt-backup`;
- confirmed the Claude Code user-scope server `zt -> C:\Users\chenx\.cargo\bin\zt.exe mcp`, the allowed `mcp__zt` tools, and the Codex `[mcp_servers.zt]` entry;
- handshook the installed server: 29 tools, listed in `integrations-install.log`.

Verification (`integrations-verified.json`): all 16 installed skill files match the repository byte for byte; the installed `zt` skill now carries the File-as-card guidance; both client configs name the installed `zt.exe`; the DSH entries are junctions to the Codex copies. Conversations that were already open keep the server they started with — a new conversation receives this contract. The MCP tool list itself did not change in this slice (no `tests`/schema addition), so the reinstall is about the guidance and the 0.21.0 binary.

Re-checked at this package's commit: the same 16 files still hash equal to `skills\` (3 + 3 + 2 files in each of `.claude\skills` and `.codex\skills`), `~/.claude.json` still answers `C:\Users\chenx\.cargo\bin\zt.exe` `mcp`, `~/.codex/config.toml` still holds `[mcp_servers.zt]` with `command = 'C:\Users\chenx\.cargo\bin\zt.exe'` and `args = ["mcp"]`, and the three `~/.dsh\skills` entries are junctions to the Codex copies.

## 9. Evidence scopes and named gaps (S28-AC05)

| Scope | What it proves | Where |
|---|---|---|
| Source tests | The code contract of each feature and the new switches | the suites in §3 |
| Frozen simulation/assembly | The two digest variants and the issue workflow still assemble and simulate | `assemble.mjs verify`; framework harness |
| Installed behavior | The actual running 0.21.0 service, its routes, MCP server and page assets | §1, §4, §5; `reverify\live-reverify.json` |
| Rollback retention | The previous build, Workflow locks and a verified archive backup exist and are re-checkable | §6 |
| Human usability | **Deferred**: the four-task sheet, Owner Save/Keep and keyboard/attention | §7 |

Named limits, unchanged or newly recorded:

1. **The human task-sheet run is deferred**; P12/P13/P15 and the Owner-Save observation are unverified. This is the only part of #121 that is not done.
2. **No live `+N` fold and no live ambiguous term** at revision 0 (the register has 78 distinct terms, 0 faults, and no restatement entry); both behaviors are proven on scratch archives, not observed here.
3. **The GUI's page-level switch behavior** is verified by the GUI half's page tests and its scratch browser pass; this package's live checks exercise the routes and `features.js` (200, served), and the human run is where the controls are finally exercised by a reader.
4. **The audit rehearsal used placeholder numbers**; it only proved that the audit accepts the sheet schema and reports P12/P13/P15. It is kept under `D:\zt-target\qualification\issue121\rehearsal\` with a README saying so, and no number from it appears in this package.
5. **The real provider pilot, the one-book run and the existing-register publication** are #117/#119 and the user's own Workflow run, not this package.

## 10. Reproduce

```powershell
cd E:\zet
$env:CARGO_TARGET_DIR='D:\zt-target';            cargo test --all-targets --locked
$env:CARGO_TARGET_DIR='D:\zt-check-target';      cargo test --manifest-path tools\zt-check\Cargo.toml --locked
$env:CARGO_TARGET_DIR='D:\zt-digest-target';     cargo test --manifest-path tools\zt-digest\Cargo.toml --locked
pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 4 -Zt D:\zt-target-121-dbg\debug\zt.exe
node "$HOME\.claude\skills\workflow-framework-protocol\tests\run-tests.mjs"
node scripts\issue-dev\tests\issue-relay.tests.mjs
node "$HOME\.agents\skills\project-wiki\scripts\wiki.mjs" check
$A = "$HOME\.claude\skills\workflow-framework-protocol\tools\assemble.mjs"
node $A verify zt-econ-digest --scope project --project E:\zet
node $A verify zt-math-digest --scope project --project E:\zet
node $A verify zt-issue-dev --scope project --project E:\zet
pwsh -NoProfile -File D:\zt-target\qualification\issue121\reverify-live.ps1   # the live route/switch/MCP checks
```

The live archive (`D:\zt`) was never a write target of the agent's checks; the only writes to it in this slice are the two Owner actions the deferred human run will make, and the verified backup precedes them. `reverify-live.ps1` reads the running service and toggles the three `config.toml` switches back to `on`; it writes no Card.
