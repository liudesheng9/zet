<#
.SYNOPSIS
  The Cited stages' deterministic command chains, one step per call (#221). A DeepSeek drive job runs one
  step and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File cited-steps.ps1 -Step list|list-finish|start|finish -Out <staging> [-PerJob 4] [-Heading <h>]

  list         plan the Cited list (parse the whole source's reference list, index the citing lines; it
               registers cited/book@1 in a prepared run) -> cited\normalize\jobs.json for the entries the
               parser could not split
  list-finish  claim cited/book@1, write the Cited list Card under the claim (-Apply -Token) and complete the
               unit from the apply report; without a unit (no ledger, or the unit is done) write it without one
  start        release a stopped run's Cited Card claims, plan the writer jobs, claim every ready job unit and
               list the claimed jobs -> drive\jobs.json (without a ledger, every job)
  finish       check every draft, complete each claimed job from the readiness result (fail the rest), then
               publish: with the book unit claimed it completes from the report, else the ready prefix is
               published without a claim

  Each step prints ONE compact JSON line (#180): `ok` first, then the counts and file paths the module reads,
  at most 400 characters; the full data goes to <staging>\cited\drive-lines\<step>.json, named as `detail`.
  Files stay under <staging>\cited\drive.
#>
param(
    [Parameter(Mandatory)][ValidateSet('list', 'list-finish', 'start', 'finish')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [int]$PerJob = 0,
    [string]$Heading = '',
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'step-line.ps1')
$Out = $Out.TrimEnd('\', '/')
$Work = Join-Path $Out 'cited'
$Drive = Join-Path $Work 'drive'
New-Item -ItemType Directory -Force $Drive | Out-Null
$LineName = $Step
$HasLedger = Test-Path -LiteralPath (Join-Path $Out 'ledger.sqlite3')

# One command: its last JSON line (or $null) and its exit code.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = (@($text) | Select-Object -Last 3) -join ' | ' }
}
function Save($Name, $Value) { ConvertTo-Json -InputObject @($Value) -Depth 64 | Set-Content -LiteralPath (Join-Path $Drive $Name) -Encoding utf8 }
function Load($Name) { $p = Join-Path $Drive $Name; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64 } }
function Reply($Value, $Detail = $null) { Send-StepLine -Line $Value -Detail $Detail -Name $LineName -Out $Work }
function Fail($Step, $Reason, $Detail = $null) { Send-StepFail -Step $Step -Reason $Reason -Name $LineName -Out $Work -Detail $Detail }
function Fail-Unit($Unit, [string]$Cause) {
    $fault = @{ schema_version = 1; unit = $Unit.id; input_hash = $Unit.input_hash; cause = $Cause.Substring(0, [Math]::Min(400, $Cause.Length)) } | ConvertTo-Json -Compress
    Run @('ledger', 'fail', '-Out', $Out, '-Unit', $Unit.id, '-Token', "$($Unit.token)", '-Fault', $fault) | Out-Null
}
# The one unit of a layer this step asked for; any other unit `next` hands out is released at once.
function Claim-One([string]$Layer, [string]$Pattern, [string]$Owner) {
    $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', $Layer, '-Take', '1', '-Owner', $Owner, '-Wait', '60', '-Lease', '7200')
    foreach ($x in @($claim.json.units | Where-Object { $_.id -notlike $Pattern })) {
        Run @('ledger', 'reclaim', '-Out', $Out, '-Unit', $x.id, '-Release') | Out-Null
    }
    @($claim.json.units | Where-Object { $_.id -like $Pattern }) | Select-Object -First 1
}

switch ($Step) {
    'list' {
        $planArgs = @('cited', '-Step', 'plan', '-Staging', $Out)
        if ($Heading) { $planArgs += @('-Heading', $Heading) }
        $plan = Run $planArgs
        if (-not $plan.json -or $plan.json.ok -ne $true) { Fail 'plan' $plan.tail; break }
        Reply @{ ok = $true; rows = [int]$plan.json.rows; unparsed = [int]$plan.json.unparsed; jobs = [int]$plan.json.jobs; file = "$($plan.json.file)" } @{ plan = $plan.json }
    }
    'list-finish' {
        $unit = if ($HasLedger) { Claim-One 'cited' 'cited/book@*' 'cited list' } else { $null }
        if ($unit) {
            $a = Run @('cited', '-Step', 'apply', '-Staging', $Out, '-Token', "$($unit.token)", '-Apply')
            if (-not $a.json -or $a.json.ok -ne $true) { Fail-Unit $unit "the Cited list apply failed: $($a.tail)"; Fail 'apply' $a.tail; break }
            $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $unit.id, '-Token', "$($unit.token)", '-Outputs', (Join-Path $Work 'apply.json'))
            if ($c.code -ne 0) { Fail-Unit $unit "the Cited list completion was refused: $($c.tail)"; Fail 'done' $c.tail; break }
            $state = 'done'
        } else {
            $a = Run @('cited', '-Step', 'apply', '-Staging', $Out, '-Apply')
            if (-not $a.json -or $a.json.ok -ne $true) { Fail 'apply' $a.tail; break }
            $state = 'unclaimed'
        }
        Reply @{ ok = $true; rows = [int]$a.json.rows; card = "$($a.json.card)"; refused = [int]$a.json.refused; unit = $state } @{ apply = $a.json; unit = $unit }
    }
    'start' {
        if ($HasLedger) {
            # A start begins the branch: a Cited Card claim still held belongs to a stopped run. Release it.
            $status = Run @('ledger', 'status', '-Out', $Out, '-Layer', 'citedcards')
            foreach ($u in @($status.json.units | Where-Object { $_.state -eq 'claimed' })) {
                Run @('ledger', 'reclaim', '-Out', $Out, '-Unit', $u.id, '-Release') | Out-Null
            }
        }
        $planArgs = @('cited-cards', '-Step', 'plan', '-Staging', $Out)
        if ($PerJob -gt 0) { $planArgs += @('-PerJob', "$PerJob") }
        $plan = Run $planArgs
        if (-not $plan.json -or $plan.json.ok -ne $true) { Fail 'plan' $plan.tail; break }
        $units = @()
        $jobArgs = @('cited-cards', '-Step', 'jobs', '-Staging', $Out)
        if ($HasLedger) {
            $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'citedcards', '-Take', '64', '-Owner', 'cited cards', '-Lease', '7200')
            # The book unit is claimed by finish, once every job is done.
            foreach ($x in @($claim.json.units | Where-Object { $_.id -like 'citedcards/book@*' })) {
                Run @('ledger', 'reclaim', '-Out', $Out, '-Unit', $x.id, '-Release') | Out-Null
            }
            $units = @($claim.json.units | Where-Object { $_.id -like 'citedcards/t*' })
            Save 'units.json' $units
            if (-not $units.Count) { Save 'jobs.json' @(); Reply @{ ok = $true; jobs = 0; plan_revision = [int]$plan.json.revision } @{ units = @(); plan = $plan.json }; break }
            $jobArgs += @('-Tokens', (($units | ForEach-Object { "$($_.inputs.job.id):$($_.token)" }) -join ','))
        } else {
            Save 'units.json' @()
        }
        $jobs = Run $jobArgs
        if (-not $jobs.json -or $jobs.json.ok -ne $true) { Fail 'jobs' $jobs.tail; break }
        Save 'jobs.json' @($jobs.json.jobs)
        Reply @{ ok = $true; jobs = @($jobs.json.jobs).Count; plan_revision = [int]$plan.json.revision; file = (Join-Path $Drive 'jobs.json') } @{ units = $units; plan = $plan.json }
    }
    'finish' {
        $check = Run @('cited-cards', '-Step', 'check', '-Staging', $Out)
        if (-not $check.json -or $check.json.ok -ne $true) { Fail 'check' $check.tail; break }
        $done = @(); $failed = @()
        foreach ($u in @(Load 'units.json')) {
            if (-not $u) { continue }
            $d = Run @('ledger', 'done', '-Out', $Out, '-Unit', $u.id, '-Token', "$($u.token)", '-Outputs', (Join-Path $Out 'cited-cards\check\readiness.json'))
            if ($d.code -eq 0 -and $d.json -and $d.json.state -eq 'done') { $done += $u.inputs.job.id }
            else { $failed += $u.inputs.job.id; Fail-Unit $u "the job $($u.inputs.job.id) is not ready: $($d.tail)" }
        }
        $bu = if ($HasLedger) { Claim-One 'citedcards' 'citedcards/book@*' 'cited cards book' } else { $null }
        if ($bu) {
            $a = Run @('cited-cards', '-Step', 'apply', '-Staging', $Out, '-Token', "$($bu.token)", '-Apply')
            if ($a.json -and $a.json.complete -eq $true) {
                $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $bu.id, '-Token', "$($bu.token)", '-Outputs', (Join-Path $Out 'cited-cards\report.json'))
                $bookState = if ($c.code -eq 0) { 'done' } else { 'failed' }
                if ($c.code -ne 0) { Fail-Unit $bu "the Cited Card book completion was refused: $($c.tail)" }
            } else { $bookState = 'failed'; Fail-Unit $bu "the publication is not complete: $($a.tail)" }
        } else {
            # A job that is not ready leaves the book unit blocked: the ready prefix is still published.
            $a = Run @('cited-cards', '-Step', 'apply', '-Staging', $Out, '-Apply')
            $bookState = if ($a.json -and $a.json.complete -eq $true -and -not $HasLedger) { 'done' } else { 'incomplete' }
        }
        $publishError = if (-not $a.json) { "$($a.tail)".Trim() } elseif ($a.json.ok -eq $false) { "$($a.json.error)".Trim() } else { '' }
        if ($publishError.Length -gt 160) { $publishError = $publishError.Substring($publishError.Length - 160) }
        Reply @{
            ok = $true; done = $done.Count; failed = $failed.Count; ready = [int]$check.json.ready; not_ready = [int]$check.json.not_ready
            book = $bookState; complete = ($a.json.complete -eq $true); rows = [int]$a.json.rows; committed = [int]$a.json.committed; publish_error = $publishError
        } @{ done = $done; failed = $failed; apply = $a.json; check = $check.json }
    }
}
