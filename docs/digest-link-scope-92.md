# The Link pass stays inside the digested book (issue #92)

This is the fix package for [#92](https://github.com/liudesheng9/zet/issues/92). The authorized #90 pilot (`zt-econ-digest` 4.4.0, mode `full`, `D:\zt-digest\pilot-90`, run `wf_6236ebf5-743`) wrote 109 authored Cards outside the book it digested. This slice removes the cause, gates every downstream group layer on the group's own ledger state, makes a claim outlive the provider run it starts, and proves all of it at the command, checker and workflow boundary. The live archive was restored before this slice began; nothing here writes a live Card, starts a provider run or runs a digest workflow.

## 1. What was wrong

Two defects produced one wrong outcome; a third made a healthy run look broken.

| Defect | Where | Effect in the pilot |
|---|---|---|
| `prepare-link` inferred ownership from body text | `tools/zt-digest/src/prepare.rs` | No digest group completed, so no group had an `owned-gNN.txt` receipt. The fallback walked every agent-Workspace Card and assigned a Card to group `n` when its address was under `<key>/<n>|…`, when it cited `来源：[[<key>]] §<section>`, **or when its body merely mentioned `§<section>` with no book key at all**. The third branch matched Cards of every book (for example every `来源：[[stiglitz2014learning]] §1.3` Card) and gave them to a pilot group. The allow-lists were wrong; `zt-digest call -Allow` then enforced them faithfully. |
| A group with no completed digest was still worked | `zt-digest-lib.js` / `zt-link-pass.js` | The Link pass ran a DeepSeek job for every group `prepare-link` listed as pending. With no completed digest output there is nothing legitimate to link, so the job acted on the inferred Cards. |
| A group job outlived its 120-second claim lease | `zt-digest/src/ledger.rs` (`ledger next`) | The group jobs ran 366–370 s. The claim expired at 120 s, an operator reclaim moved the token forward, and each job then hit `stale_claim` in its SELF-CHECK, so `g001` and `g003` never completed either. |

## 2. The fix

### 2.1 Ownership is receipt-based (`prepare-link`)

A group now owns exactly:

- the addresses in its own `owned-gNN.txt` (the helper's `-Log` during the digest, republished by `group-finish` from its checked results);
- its anchor Card;
- a Card of the digested book's **own** Literature tree under that anchor (`<key>/<n>|…`) that carries a source mark.

The `names_section` and `cites_section` fallbacks and their section-id helpers are removed. No Card of another book and no Card of an existing Topic tree can be inferred from body text, which is issue #92's acceptance criterion 1 in its "removed in favour of ledger receipts" form.

### 2.2 A group without completed digest output gets no Link job

`prepare-link` reads the staging folder's `ledger.sqlite3` and takes each group's `digest/gNNN@R` state:

- `done` → the group is eligible: its allow-list holds its owned Cards and it may be pending;
- any other state (`ready`, `claimed`, `failed`, `blocked`, `unregistered`) → the group is **blocked**: `allow-gNN.txt` is written empty, `link-gNN.md` states that the group's digest did not complete, and the group is not pending;
- a folder with no ledger (a preparation with no declared variant, or a legacy folder) → its `owned-gNN.txt` receipts are the only evidence; a missing receipt is `unverified` and blocked the same way.

The JSON gains `blocked: [{n, unit, state, reason}]`, per-group `digest`/`blocked` fields and `digest_state: "ledger" | "receipts"`.

`zt-link-prepare` v4 returns and logs the blocked groups; `zt-link-pass` v3 runs jobs only for the pending ones and passes the blocked list through; `zt-digest-report` v10 carries it as `link_blocked`, so the run's result names the blocked closure beside the ledger report's own `gaps`/`unresolved`.

### 2.3 A claim covers the provider run it starts

- `ledger next` takes `-Lease <1..7200>`; the claim's lease is the longer of the base 120 seconds and the request.
- The workflow claims with `dshRunCap + 300` (1500 s at the frozen default 1200 s), so a six-minute group job cannot be re-readied under its own claim.
- A claimed chain now polls with `wait_seconds 90` and runs `ledger renew` with every wait round, so a chain of several provider runs keeps the claim current between them. The 30-second internal heartbeat still covers every long command.

## 3. Regression tests (test-first)

The two suites the issue asks for, plus the ownership suite, were written first and failed on the installed 4.4.0-era binaries. All three fail against the previous binary and pass against this build; the RED and GREEN logs are retained.

| Suite | RED on `zt-digest` `21022965…` | GREEN on this build |
|---|---|---|
| `prepare-link-scope.tests.ps1` (new, 24 assertions) | **12 failed**: every group pending with no completed digest, no `blocked` list, and a failed group owning `0/1`, `Other2020/1`, `ScopeBook/2` — a foreign Topic tree's Card and another book's Cards, exactly the pilot's defect | all passed |
| `prepare.tests.ps1` (32 assertions, extended) | **2 failed**: no `digest_state`, and a group with no completion receipt still pending | all passed |
| `ledger-claims.tests.ps1` (53 assertions, extended) | **error**: `unknown parameter -Lease` | all passed |

The new suite drives the real commands on a prepared economics run of the pilot source (`docs/digest-pilot-short-source.zh.md`, 3 groups, SHA-256 `d0a27cd8…`):

1. prepare binds three groups and creates the run's ledger;
2. a second book (`Other2020`) and an existing Topic tree are created, with Cards that cite `来源：[[ScopeBook]] §1.2` / `§2.1` and Cards whose bodies merely mention `§1.2`/`§2.1`;
3. with no completed digest, no group is pending, all three are blocked (`1:ready, 2:ready, 3:ready`), every allow-list is empty and every link file says the digest did not complete;
4. group 1 completes through the real `group-start` → draft → `group-finish` lifecycle and becomes the only pending group, owning exactly `ScopeBook/1` and its new Card — never the other book's Cards and never the Topic tree's Card;
5. group 2 records a real `check_failed`/`content` failure, and an `owned-g002.txt` naming `Other2020/1` and `0/1` cannot make them owned: the group stays blocked with an empty allow-list and the report names groups 2 and 3;
6. `linked-g01.txt` takes the finished group off the pending list.

The lease test in `ledger-claims.tests.ps1` claims `terms/book@1` with `-Lease 60` over a 2-second base lease, waits 3 seconds, and proves the claim is still `claimed` under its original token, still renews, and refuses `ledger reclaim` with `claim_active`; the same unit, expired by the suite's own lease injection, is then the only one the guarded reclaim takes. `-Lease 0` and `-Lease 99999` are refused with exit 2.

## 4. Verification

On Windows, 2026-10-18, against the candidate then the installed binaries:

| Check | Result | Log |
|---|---|---|
| Root, all targets, locked | **376 passed, 0 failed, 1 intentionally ignored** (the manual real-clipboard check) | `D:\zt-target` run |
| `tools/zt-check` | unchanged crate, 10 unit tests | — |
| `tools/zt-digest` release, locked | **133 passed, 0 failed** (the new `claim_lease` unit test) | — |
| Aggregate public runner (candidate `zt-digest.exe`) | **64 suites, 0 failed, 2,024 assertions, 306 s** | `D:\zt-digest-target\qualification\issue92\public-summary.log` |
| Installation smoke | **9 assertions, 0 failed** on the installed set | `install-smoke.tests.ps1` |
| `prepare-link-scope` on the installed set | **24 assertions, 0 failed** | `green\prepare-link-scope.log` |
| Framework harness (both variants, stand-in runtime) | **177 checks, 0 failed** — the covering lease on every claim, the renewal instruction, no link job for a blocked group, `link_blocked` in the report | `framework-harness.mjs` |
| Bridge workflow harness | **2 passed** | `bridge-workflow-harness.mjs` |
| Framework protocol own tests | **28 passed, 0 failed** | `run-tests.mjs` |
| Workflow tools, issue relay | **16 scenarios passed** | `issue-relay.tests.mjs` |
| Frozen installs | `verify`: `zt-econ-digest` **4.5.0**, `zt-math-digest` **3.5.0**, `zt-issue-dev` **1.1.1** — no problems, no notes, no library drift; both dry runs `{"ok":true,"halted":null}` | — |
| Developer wiki | the block docs are updated in the same commit | — |

Nothing in this slice runs the workflow itself: the frozen `zt-econ-digest.js` is installed and verified, and its stand-in run reaches `halted: null`. The issue's criterion 5 (run the #90 pilot again from a fresh staging folder) is the user's authorized Claude Code run and stays outside this slice; it must now run `zt-econ-digest` 4.5.0 with the same args (a fresh `out`, `mode: full`), and its expected behaviour is: the injected `check_failed` group stays `failed`, **no** Link job runs for it, its allow-list is empty, its `link_blocked` entry exists, and a six-minute group job completes under its own claim.

## 5. Installed identity and deployment

| Artifact | Source delta | Installed SHA-256 |
|---|---|---|
| `zt.exe` | none (`src/` unchanged since the deployed release) | `751727E77F27F68466018F7E0003773363FFBA49F3B0EBE4A9BAF2DE831624E9`, byte-identical to a fresh `cargo build --release --locked` of this HEAD |
| `zt-boot.exe` | none | `BCE8F8966EA41FB5900441A14F45A0A0F1C82144437CCC9869774A941E42EE89`, byte-identical to the fresh build |
| `zt-check.exe` | none (`tools/zt-check/` unchanged) | `36D0FD6195C0867746B87F97A2C06971E5BFDFDF112EACAFC08AC13E44F30A2F` |
| `zt-digest.exe` | this slice | `D6A01A71EF97F8012A663C11BA9EDC1D2D8D1B5534805B653F57A6D875FF039C`, equal to the release artifact at `D:\zt-digest-target\release\zt-digest.exe`; installed by `scripts\install-zt-digest.ps1`, previous image kept at `D:\zt-target\rollback\zt-digest-pre-issue92-*.exe` |

The service stayed up on the unchanged `zt.exe` (daemon 45440 on `C:\Users\chenx\.cargo\bin\zt.exe`): `zt status` reports `state: up`, archive `D:\zt`, 10,895 Cards, 0 Sessions, revision 0, `daemon: version=0.20.0`; `zt status --check-links` reports no fault. No live Card write, archive migration or digest workflow ran.

## 6. Limits

- The Link pass still writes through the helper and its `link/gNNN@1` units still settle as explicit `unsupported_claim` content failures: the receipt-bound Link draft transport (R263's Link half, #82) is unchanged here. This slice makes the *scope* of those writes correct, not their completion evidence.
- Whole-book layers (Organize, Figures, Hubs, Bridge, the audit, the finalization) still run on whatever the completed groups produced, as #90 §7.2 intends; they now cannot see a link scope that never existed, and the report names the blocked groups beside the ledger's own blocked/unsettled closure.
- The covering lease trades fast expiry for correctness: a claim whose holder dies now stays claimed for up to `dshRunCap + 300` seconds. The guarded `ledger reclaim` remains the only path to a replacement attempt, and a run that loses a manager still names that unit as unresolved.
- `prepare-link` reads the ledger read-only; a folder whose ledger is being written at that moment may need one rerun, which is safe because it changes no Card.
