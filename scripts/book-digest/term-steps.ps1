<#
.SYNOPSIS
  The Term stages' deterministic command chains, one step per call (#172). A DeepSeek job runs one
  step and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File term-steps.ps1 -Step start|review|fix|finish|link -Out <staging> [-Occurrences first|all]

  start   plan the Term cards, claim every ready job unit, list the claimed jobs -> drive\jobs.json
  review  check the drafts and plan the reviews                               -> drive\reviews.json
  fix     plan the one fix per job                                            -> drive\fixes.json
  finish  complete each claimed job from the readiness result (fail the rest), then publish:
          with the book unit claimed it completes from the report, else the ready prefix is published
  link    plan the Term links, apply under the claim (or, when the unit is blocked, the published
          prefix without a claim), then plan again: the second plan must change no Card

  Each step prints one compact JSON line. Files stay under <staging>\term-cards\drive.
#>
param(
    [Parameter(Mandatory)][ValidateSet('start', 'review', 'fix', 'finish', 'link')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [ValidateSet('first', 'all')][string]$Occurrences = 'first',
    [int]$PerJob = 0,
    [int]$MathPerJob = 0,
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
$Out = $Out.TrimEnd('\', '/')
$Work = Join-Path $Out 'term-cards'
$Drive = Join-Path $Work 'drive'
New-Item -ItemType Directory -Force $Drive | Out-Null

# One command: its last JSON line (or $null) and its exit code.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = (@($text) | Select-Object -Last 3) -join ' | ' }
}
function Save($Name, $Value) { $Value | ConvertTo-Json -Depth 64 | Set-Content -LiteralPath (Join-Path $Drive $Name) -Encoding utf8 }
function Load($Name) { $p = Join-Path $Drive $Name; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64 } }
function Reply($Value) { $Value | ConvertTo-Json -Depth 16 -Compress; exit 0 }
function Fail-Unit($Unit, [string]$Cause) {
    $fault = @{ schema_version = 1; unit = $Unit.id; input_hash = $Unit.input_hash; cause = $Cause.Substring(0, [Math]::Min(400, $Cause.Length)) } | ConvertTo-Json -Compress
    Run @('ledger', 'fail', '-Out', $Out, '-Unit', $Unit.id, '-Token', "$($Unit.token)", '-Fault', $fault) | Out-Null
}

switch ($Step) {
    'start' {
        $planArgs = @('term-cards', '-Step', 'plan', '-Staging', $Out)
        if ($PerJob -gt 0) { $planArgs += @('-PerJob', "$PerJob") }
        if ($MathPerJob -gt 0) { $planArgs += @('-MathPerJob', "$MathPerJob") }
        $plan = Run $planArgs
        if (-not $plan.json -or $plan.json.ok -ne $true) { Reply @{ ok = $false; step = 'plan'; error = $plan.tail } }
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termcards', '-Take', '64', '-Owner', 'term cards', '-Lease', '14400')
        $units = @($claim.json.units | Where-Object { $_.id -like 'termcards/t*' })
        Save 'units.json' $units
        if (-not $units.Count) { Save 'jobs.json' @(); Reply @{ ok = $true; jobs = 0; plan_revision = $plan.json.revision } }
        $tokens = ($units | ForEach-Object { "$($_.inputs.job.id):$($_.token)" }) -join ','
        $jobs = Run @('term-cards', '-Step', 'jobs', '-Staging', $Out, '-Tokens', $tokens)
        if (-not $jobs.json -or $jobs.json.ok -ne $true) { Reply @{ ok = $false; step = 'jobs'; error = $jobs.tail } }
        Save 'jobs.json' @($jobs.json.jobs)
        Reply @{ ok = $true; jobs = @($jobs.json.jobs).Count; plan_revision = $plan.json.revision; file = (Join-Path $Drive 'jobs.json') }
    }
    'review' {
        Run @('term-cards', '-Step', 'check', '-Staging', $Out) | Out-Null
        $r = Run @('term-cards', '-Step', 'review-plan', '-Staging', $Out)
        if (-not $r.json -or $r.json.ok -ne $true) { Reply @{ ok = $false; step = 'review-plan'; error = $r.tail } }
        $jobs = @($r.json.jobs | Where-Object { $_.delete_after_collect -eq $true })
        Save 'reviews.json' $jobs
        Reply @{ ok = $true; reviews = $jobs.Count; counts = $r.json.counts; file = (Join-Path $Drive 'reviews.json') }
    }
    'fix' {
        $f = Run @('term-cards', '-Step', 'fix-plan', '-Staging', $Out)
        if (-not $f.json -or $f.json.ok -ne $true) { Reply @{ ok = $false; step = 'fix-plan'; error = $f.tail } }
        # A fix runs in a new conversation from its fallback files: the writers are not kept.
        $jobs = @($f.json.jobs | Where-Object { $_.continue } | ForEach-Object { $_.files = $_.fallback; $_.PSObject.Properties.Remove('continue'); $_ })
        Save 'fixes.json' $jobs
        Reply @{ ok = $true; fixes = $jobs.Count; counts = $f.json.counts; file = (Join-Path $Drive 'fixes.json') }
    }
    'finish' {
        Run @('term-cards', '-Step', 'check', '-Staging', $Out) | Out-Null
        $done = @(); $failed = @()
        foreach ($u in @(Load 'units.json')) {
            if (-not $u) { continue }
            $d = Run @('ledger', 'done', '-Out', $Out, '-Unit', $u.id, '-Token', "$($u.token)", '-Outputs', (Join-Path $Work 'check\readiness.json'))
            if ($d.code -eq 0 -and $d.json -and $d.json.state -eq 'done') { $done += $u.inputs.job.id }
            else { $failed += $u.inputs.job.id; Fail-Unit $u "the job $($u.inputs.job.id) is not ready: $($d.tail)" }
        }
        $book = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termcards', '-Take', '1', '-Owner', 'term cards book', '-Wait', '60', '-Lease', '3600')
        $bu = @($book.json.units | Where-Object { $_.id -like 'termcards/book@*' }) | Select-Object -First 1
        if ($bu) {
            $a = Run @('term-cards', '-Step', 'apply', '-Staging', $Out, '-Token', "$($bu.token)", '-Apply')
            if ($a.json -and $a.json.complete -eq $true) {
                $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $bu.id, '-Token', "$($bu.token)", '-Outputs', (Join-Path $Work 'report.json'))
                $bookState = if ($c.code -eq 0) { 'done' } else { 'failed' }
                if ($c.code -ne 0) { Fail-Unit $bu "the book completion was refused: $($c.tail)" }
            } else { $bookState = 'failed'; Fail-Unit $bu "the publication is not complete: $($a.tail)" }
        } else {
            # A failed job leaves the book unit blocked: its ready contiguous prefix is still published.
            $a = Run @('term-cards', '-Step', 'apply', '-Staging', $Out, '-Apply')
            $bookState = 'incomplete'
        }
        Reply @{ ok = $true; done = $done; failed = $failed; book = $bookState; publication = @{ complete = $a.json.complete; rows = $a.json.rows; committed = $a.json.committed; ready_unpublished = $a.json.ready_unpublished; stop = $a.json.stop } }
    }
    'link' {
        $p = Run @('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $Occurrences)
        if (-not $p.json -or $p.json.ok -ne $true) { Reply @{ ok = $false; step = 'plan'; error = $p.tail } }
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termlink', '-Take', '1', '-Owner', 'term-link', '-Wait', '60', '-Lease', '3600')
        $u = @($claim.json.units | Where-Object { $_.id -like 'termlink/*' }) | Select-Object -First 1
        if ($u) {
            $a = Run @('term-link', '-Step', 'apply', '-Staging', $Out, '-Occurrences', $Occurrences, '-Token', "$($u.token)", '-Apply')
            if ($a.json -and $a.json.complete -eq $true) {
                $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $u.id, '-Token', "$($u.token)", '-Outputs', (Join-Path $Out 'term-link\apply.json'))
                $status = if ($c.code -eq 0) { 'done' } else { 'failed' }
                if ($c.code -ne 0) { Fail-Unit $u "the Term-link completion was refused: $($c.tail)" }
            } else { $status = 'failed'; Fail-Unit $u "the Term-link pass stopped: $($a.tail)" }
        } else {
            $gate = Run @('ledger', 'gate', '-Out', $Out, '-Layer', 'termlink', '-Brief')
            if ($gate.json -and $gate.json.complete -eq $true) { Reply @{ ok = $true; status = 'skipped'; reason = 'already complete' } }
            # The unit is blocked by an incomplete Term-card book: link the published prefix, unit stays open.
            $a = Run @('term-link', '-Step', 'apply', '-Staging', $Out, '-Occurrences', $Occurrences, '-Apply')
            $status = if ($a.json -and $a.json.complete -eq $true) { 'partial' } else { 'failed' }
        }
        $second = Run @('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $Occurrences)
        Reply @{ ok = $true; status = $status; unit = $u.id; counts = $a.json.counts; second = @{ cards_changed = $second.json.cards_changed; cards_refused = $second.json.cards_refused } }
    }
}
