#Requires -Version 7.5
<#
.SYNOPSIS
  The deterministic half of the card layer cycle (docs/digest-card-layers-proposal.md, proposals 3,
  4, and 6): check the Idea Cards, Event Cards, and anchors of one book, hand their faults to
  DeepSeek as jobs, check what DeepSeek writes, apply it, check again, and report. DeepSeek writes
  only JSON files; this script makes every zt write. It never writes a Model Card, a Model part,
  a Hub Card, or a Topic root. This script forwards to `zt-digest card`
  (tools/zt-digest/src/card.rs); callers should call the tool directly.

.DESCRIPTION
  -Work is the cycle's folder and the DeepSeek Harness cwd. It holds state.json, the job files,
  out\ (the JSON files DeepSeek writes), before\ and after\ (digest-audit runs), report.json,
  and decisions.json. Every step prints compact JSON as its last line. A step that hands work to
  DeepSeek prints {"jobs":[{"id","files":[...],"prompt","continue"?,"fallback"?}], ...}.

    plan     runs digest-audit on -Book; picks its Idea Cards, Event Cards, and anchors (only
             the Cards of -Groups when given: owned-gNN.txt in -Staging and the anchors); writes
             one fix job per -PerJob Cards with faults of -Rules, a review job for a -Sample of
             the Cards without faults (only with -Staging: the reviewer reads the source), and one
             orphan job per -PerJob Cards that no Card other than the parent links (I5, E4).
    apply    reads out\fix-*.json and out\orphans-*.json of the jobs not applied yet. A rewrite
             must keep every Link, figure, and formula of the Card, and the Card must be unchanged
             since the plan. It writes the rewrites (edit_card with base), the splits (a Side
             successor), and the orphan Links (append_card on a candidate, with a kind word before
             the Link). merge verdicts go to decisions.json; a refusal goes to the next check.
    check    runs digest-audit again; prints one follow-up job per fix job whose Cards still
             have faults of -Rules or were refused. The follow-up continues the job's conversation.
    report   runs digest-audit again; compares the faults of the picked Cards before and
             after, per rule; writes report.json and decisions.json.
    precheck reads the JSON file of one job (-Job; -File, or the job's `out` in state.json) and
             checks it the way apply and the audit will, without a write: no zt call, no change to
             state.json. Every job file names this command in its `SELF-CHECK:` line, so DeepSeek
             checks its draft before it replies. A fix entry: a known verdict, an address of the
             job, a writable category, the rewrite test (every Link, figure, and formula kept, no
             Link to an unknown Card), then the Card language and layer rules of -Rules on the new
             title and body and on each split Card (zt-check --json, with the Link kinds
             as ZT reads them). Rules that need the whole Workspace are left out: I5, E4, I6, E5,
             and L3 (advisory). An orphan entry: the orphan and the candidate belong to the job, the
             text links the orphan with a kind word before the Link, no semicolon, no Link to an
             unknown Card, and L2 (with the R rules and L1 of -Rules) on the sentence. Output: one
             line per fault, `<address> <rule or check> <text>`, then `faults: N` as the last line.
             The check `refuse` names what apply refuses; a split Card is `<address>+split<n>`; an
             orphan Link is `<orphan><-<candidate>`.

  Parameters: -Step plan|apply|check|report|precheck and -Work <folder> (required); -Book,
  -Staging, -Groups, -PerJob 8, -Sample 0, -Seed 1, -Rules (default
  R2,R3,R4,R5,R7,R9,R12,R18,R19,R20,R21,R22,I1,I2,I3,I4,I7,L1,L2,E1,E2: R6 is left out), -MaxCards 0, -NoOrphans,
  -Notes (Organize notes: each note is a fault N1 of its Card), -Compact (compact job lists),
  -MaxJobs 0 (apply: at most this many jobs per call), -Job and -File (precheck).

  This script forwards every argument to `zt-digest card`, which does the work in one process
  with one `zt mcp`, the audit and zt-check in-process. It sets ZT_DIGEST_SCRIPTS to this folder:
  the tool reads the instruction files (dsh-card-instructions.zh.md, dsh-orphan-instructions.zh.md)
  from it and names this script in each job's SELF-CHECK line. ZT_DIGEST names another zt-digest
  build.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the instruction files and of this script (the SELF-CHECK line names it).
$env:ZT_DIGEST_SCRIPTS = $PSScriptRoot
& $exe card @args
exit $LASTEXITCODE
