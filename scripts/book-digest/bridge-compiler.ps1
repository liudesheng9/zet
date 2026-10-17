#Requires -Version 7.5
<#
.SYNOPSIS
  Plan, check, publish, and explicitly apply report-first cross-book Bridges.
  Every step prints compact JSON last. Plan, precheck, report, and preview never edit Cards.

  This script forwards to `zt-digest bridge` (tools/zt-digest, installed beside zt.exe by
  scripts/install-zt-digest.ps1), which does the work in one process. Call `zt-digest bridge`
  directly when you can: it takes the same parameters, writes the same files, and prints the same
  lines.

.DESCRIPTION
  Parameters: -Step* (plan, precheck, report, apply), -Staging*, -CitationKey*, -Work (default
  <Staging>\bridge), -Profile (book or math; default book), -MaxSources (1..120; default 60),
  -Candidates (1..8; default 5), -Books (a comma-separated target-book filter), -Job (precheck),
  -ReportId, -Accept, -AcceptEarlier (ids, or all), -Apply.

    plan      freezes the source Cards of -CitationKey and their candidate Cards in other books in
              <Work>\plan.json and writes one judge job per twelve sources (jobs\br-NNN.md). Each
              job names its self-check: zt-digest bridge -Step precheck ... -Job br-NNN.
    precheck  checks one judge answer (out\<Job>.json) against the frozen plan: BR1-BR9 fault
              lines, then `faults: N`.
    report    checks every answer and publishes the immutable proposals.json and report.md.
    apply     previews the accepted proposals of -ReportId; with -Apply writes them through guarded
              edits, journals them in state.json, and verifies them.

  A refusal of the input exits 1, an operational failure 2; both print {"ok":false,...} last.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# ZT_DIGEST names another build (a test of an unreleased zt-digest).
$exe = if ($env:ZT_DIGEST) { $env:ZT_DIGEST } else { (Get-Command zt-digest -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $exe) { throw 'zt-digest is not installed: run scripts\install-zt-digest.ps1' }
# The folder of the judge instructions (dsh-bridge-instructions.zh.md).
$env:ZT_DIGEST_SCRIPTS = $PSScriptRoot
# `pwsh -File` and `&` hand `-Name:value` over as two arguments (a switch value as a Boolean) and a
# list as an array; the tool takes `-Name:value` and a comma-separated list.
$pass = [Collections.Generic.List[string]]::new()
foreach ($a in $args) {
    $last = if ($pass.Count) { $pass[$pass.Count - 1] } else { '' }
    if ($last -match '^-[A-Za-z]\w*:$') { $pass[$pass.Count - 1] = $last + $(if ($a -is [array]) { $a -join ',' } else { "$a" }) }
    elseif ($a -is [bool] -and $last -match '^-[A-Za-z]\w*$') { $pass[$pass.Count - 1] = "${last}:$a" }
    elseif ($a -is [array]) { $pass.Add($a -join ',') }
    else { $pass.Add("$a") }
}
& $exe bridge @pass
exit $LASTEXITCODE
