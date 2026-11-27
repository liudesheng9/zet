<#
.SYNOPSIS
  The Cited stages' deterministic command chains, one step per call (#221). A DeepSeek drive job runs one
  step and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File cited-steps.ps1 -Step list|list-finish|start|finish -Out <staging> [-PerJob 4] [-Heading <h>]
                                        [-Source <the whole bilingual source, when the manifest's source is not it>]

  list         (-SkipDone: a finished list answers settled and plans nothing) plan the Cited list (mark the reference material and cut it into parts; it registers
               cited/book@1 in a prepared run) -> cited\read\jobs.json, one DeepSeek reading job per four
               parts (one message a part); a job whose parts all pass is not listed again
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
    [ValidateRange(1, 8)][int]$Chunk = 8,
    [string]$Heading = '',
    [string]$Source = '',
    [switch]$SkipDone,
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
function Fail($Step, $Reason, $Detail = $null, $Extra = $null) { Send-StepFail -Step $Step -Reason $Reason -Name $LineName -Out $Work -Detail $Detail -Extra $Extra }
function Planning-Stop($RunResult) {
    if ($RunResult -and $RunResult.json) {
        $code = ''
        if ("$($RunResult.json.code)" -in @('card_locked', 'scope_refused')) { $code = "$($RunResult.json.code)" }
        elseif ("$($RunResult.json.error)" -in @('card_locked', 'scope_refused')) { $code = "$($RunResult.json.error)" }
        if ($code) {
            $ref = if ($RunResult.json.refusal) { $RunResult.json.refusal } else { $RunResult.json }
            return @{
                code = $code
                message = if ($ref.message) { "$($ref.message)" } elseif ($RunResult.json.message) { "$($RunResult.json.message)" } else { "$($RunResult.tail)" }
                tree = "$($ref.tree)"
                lock_id = $ref.lock_id
                refusal = $ref
            }
        }
    }
    return $null
}
function Fail-PlanningStop([string]$Where, $StopObj, $Detail = $null) {
    $code = if ($StopObj.code) { "$($StopObj.code)" } else { 'card_locked' }
    Fail $Where "${code}: $($StopObj.message)" $Detail @{ code = $code; message = "$($StopObj.message)"; tree = "$($StopObj.tree)"; lock_id = $StopObj.lock_id }
}
function Validate-DoneBatch($RunResult, [object[]]$ExpectedUnits) {
    if (-not $RunResult -or -not $RunResult.json -or $RunResult.json.kind -ne 'ledger_done_batch' -or -not ($RunResult.json.results -is [array])) {
        return @{ ok = $false; map = @{}; stop_reason = '' }
    }
    if ("$($RunResult.json.stop_reason)" -ne '') {
        return @{ ok = $false; map = @{}; stop_reason = "$($RunResult.json.stop_reason)" }
    }
    $map = @{}
    foreach ($r in @($RunResult.json.results | Where-Object { $_ -and $_.unit -and $null -ne $_.token })) {
        $map["$($r.unit):$($r.token)"] = $r
    }
    foreach ($u in @($ExpectedUnits)) {
        if (-not $map.ContainsKey("$($u.id):$($u.token)")) {
            return @{ ok = $false; map = $map; stop_reason = '' }
        }
    }
    return @{ ok = $true; map = $map; stop_reason = '' }
}
function Reconcile-Units([object[]]$TargetUnits) {
    $ids = (@($TargetUnits | ForEach-Object { "$($_.id)" }) -join ',')
    $s = Run @('ledger', 'status', '-Out', $Out, '-Units', $ids)
    $map = @{}
    if ($s.json -and $s.json.units -is [array]) {
        foreach ($u in @($s.json.units | Where-Object { $_ -and $_.id })) { $map["$($u.id)"] = $u }
    }
    return $map
}
function Fail-Unit($Unit, [string]$Cause) {
    $fault = @{ schema_version = 1; unit = $Unit.id; input_hash = $Unit.input_hash; cause = $Cause.Substring(0, [Math]::Min(400, $Cause.Length)) } | ConvertTo-Json -Compress
    Run @('ledger', 'fail', '-Out', $Out, '-Unit', $Unit.id, '-Token', "$($Unit.token)", '-Fault', $fault) | Out-Null
}
# The one unit of a layer this step asked for, selected by literal prefix inside SQLite.
function Claim-One([string]$Layer, [string]$Prefix, [string]$Owner) {
    $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', $Layer, '-Prefix', $Prefix, '-Take', '1', '-Owner', $Owner, '-Wait', '60', '-Lease', '7200')
    @($claim.json.units | Where-Object { $_ -and "$($_.id)".StartsWith($Prefix) }) | Select-Object -First 1
}

# v3 (atkinson2015lectures, 2026-11-02): a manager told to run "entries 232 through 239 (0-based)" of a job list ran the
# job ids t232..t239 instead. Each manager gets its own chunk file of at most -Chunk entries (the DSMP manager size).
function Save-Chunks([string]$Prefix, [object[]]$Items) {
    Get-ChildItem -LiteralPath $Drive -Filter "$Prefix-*.json" -ErrorAction SilentlyContinue | Remove-Item -Force
    $n = [int][Math]::Ceiling(@($Items).Count / $Chunk)
    for ($k = 0; $k -lt $n; $k++) { Save ('{0}-{1:D3}.json' -f $Prefix, ($k + 1)) @($Items | Select-Object -Skip ($k * $Chunk) -First $Chunk) }
    $n
}

switch ($Step) {
    'list' {
        # scheiner2022baghdad (2026-11-04): a resume goes on from where the run broke, so a finished list is left alone:
        # no plan, no reading job and no list-finish, which would write the list again without a claim.
        if ($SkipDone -and $HasLedger) {
            $listed = @((Run @('ledger', 'status', '-Out', $Out, '-Layer', 'cited')).json.units)
            if ($listed.Count -and -not @($listed | Where-Object { $_.state -ne 'done' }).Count) { Reply @{ ok = $true; settled = $true }; break }
        }
        $planArgs = @('cited', '-Step', 'plan', '-Staging', $Out)
        if ($Heading) { $planArgs += @('-Heading', $Heading) }
        if ($Source) { $planArgs += @('-Source', $Source) }
        $plan = Run $planArgs
        if (-not $plan.json -or $plan.json.ok -ne $true) { Fail 'plan' $plan.tail; break }
        $chunks = 0
        if ("$($plan.json.file)" -and (Test-Path -LiteralPath "$($plan.json.file)")) {
            $chunks = Save-Chunks 'read' @(Get-Content -LiteralPath "$($plan.json.file)" -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64)
        }
        Reply @{ ok = $true; rows = [int]$plan.json.rows; parts = [int]$plan.json.parts; jobs = [int]$plan.json.jobs; chunks = $chunks; file = "$($plan.json.file)" } @{ plan = $plan.json }
    }
    'list-finish' {
        # donner2026industry (2026-11-11): a list-finish that failed (a part had no answer) left cited/book@1 failed.
        # Mode cited-cards read the list again and wrote it without a claim, and every Cited Card job stayed blocked
        # behind the failed unit. A list-finish runs only after the reading, so a failed list unit is reopened here.
        if ($HasLedger) {
            $listed = @((Run @('ledger', 'status', '-Out', $Out, '-Layer', 'cited')).json.units)
            foreach ($u in @($listed | Where-Object { $_ -and $_.state -eq 'failed' -and $_.id -like 'cited/book@*' })) {
                Run @('ledger', 'reopen', '-Out', $Out, '-Units', $u.id, '-Reason', 'the Cited list was read again; list-finish writes it under a new claim') | Out-Null
            }
        }
        $unit = if ($HasLedger) { Claim-One 'cited' 'cited/book@' 'cited list' } else { $null }
        if ($unit) {
            $a = Run @('cited', '-Step', 'apply', '-Staging', $Out, '-Token', "$($unit.token)", '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-PlanningStop 'apply' $aStop @{ unit = $unit; refusal = $aStop.refusal }; break }
            if (-not $a.json -or $a.json.ok -ne $true) { Fail-Unit $unit "the Cited list apply failed: $($a.tail)"; Fail 'apply' $a.tail; break }
            $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $unit.id, '-Token', "$($unit.token)", '-Outputs', (Join-Path $Work 'apply.json'))
            if ($c.code -ne 0) { Fail-Unit $unit "the Cited list completion was refused: $($c.tail)"; Fail 'done' $c.tail; break }
            $state = 'done'
        } else {
            $a = Run @('cited', '-Step', 'apply', '-Staging', $Out, '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-PlanningStop 'apply' $aStop @{ refusal = $aStop.refusal }; break }
            if (-not $a.json -or $a.json.ok -ne $true) { Fail 'apply' $a.tail; break }
            $state = 'unclaimed'
        }
        Reply @{ ok = $true; rows = [int]$a.json.rows; card = "$($a.json.card)"; repeated = [int]$a.json.repeated; unit = $state } @{ apply = $a.json; unit = $unit }
    }
    'start' {
        # A book with no reference list (rudin1976principles pilot, 2026-11-23): the Cited list settled with zero rows
        # and wrote no Card, so there is no Cited Card to write.
        $listApply = Join-Path $Out 'cited\apply.json'
        if (Test-Path -LiteralPath $listApply) {
            $la = Get-Content -LiteralPath $listApply -Raw -Encoding utf8 | ConvertFrom-Json
            if ($la.applied -eq $true -and [int]$la.rows -eq 0) { Reply @{ ok = $true; jobs = 0; no_list = $true } @{ list = $la }; break }
        }
        if ($HasLedger) {
            # A start begins the branch: release any Cited Card claim a stopped run held in one command.
            $rel = Run @('ledger', 'reclaim', '-Out', $Out, '-Layer', 'citedcards', '-All', '-Release')
            if ($rel.code -ne 0) { Fail 'release' $rel.tail; break }
            if ($rel.json -and (@($rel.json.blocked).Count -or @($rel.json.skipped).Count)) {
                $first = @(@($rel.json.blocked) + @($rel.json.skipped))[0]
                Fail 'release' "$($first.unit): $($first.reason)"; break
            }
        }
        $planArgs = @('cited-cards', '-Step', 'plan', '-Staging', $Out)
        if ($PerJob -gt 0) { $planArgs += @('-PerJob', "$PerJob") }
        $plan = Run $planArgs
        $pStop = Planning-Stop $plan
        if ($pStop) { Fail-PlanningStop 'plan' $pStop @{ refusal = $pStop.refusal }; break }
        if (-not $plan.json -or $plan.json.ok -ne $true) { Fail 'plan' $plan.tail; break }
        $units = @(); $settled = @()
        $jobArgs = @('cited-cards', '-Step', 'jobs', '-Staging', $Out)
        if ($HasLedger) {
            # Claim every ready Cited Card job unit in one transaction without touching citedcards/book@*.
            $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'citedcards', '-Prefix', 'citedcards/t', '-All', '-Owner', 'cited cards', '-Lease', '7200')
            if ($claim.code -ne 0 -or -not $claim.json) { Fail 'claim' $claim.tail; break }
            $units = @($claim.json.units | Where-Object { $_ -and "$($_.id)".StartsWith('citedcards/t') })
            # A restarted branch keeps its drafts: settle all already-ready jobs in one batch command.
            if ($units.Count) {
                Run @('cited-cards', '-Step', 'check', '-Staging', $Out) | Out-Null
                $readyPath = Join-Path $Out 'cited-cards\check\readiness.json'
                if (Test-Path -LiteralPath $readyPath) {
                    $rows = @((Get-Content -LiteralPath $readyPath -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64).rows)
                    $candidates = @(); $open = @()
                    foreach ($u in $units) {
                        $id = "$($u.inputs.job.id)"
                        $own = @($rows | Where-Object { "$($_.job)" -eq $id })
                        if ($own.Count -and -not @($own | Where-Object { $_.status -ne 'ready' }).Count) { $candidates += $u }
                        else { $open += $u }
                    }
                    if ($candidates.Count) {
                        Save 'settle.json' $candidates
                        $db = Run @('ledger', 'done', '-Out', $Out, '-UnitsFile', (Join-Path $Drive 'settle.json'), '-Outputs', $readyPath)
                        $valid = Validate-DoneBatch $db $candidates
                        if ($valid.ok) {
                            foreach ($u in $candidates) {
                                $r = $valid.map["$($u.id):$($u.token)"]
                                if ($r.status -eq 'done' -or $r.ledger_state -eq 'done') { $settled += "$($u.inputs.job.id)" }
                                else { $open += $u }
                            }
                        } else {
                            $states = Reconcile-Units $candidates
                            foreach ($u in $candidates) {
                                $st = $states["$($u.id)"]
                                if ($st -and ($st.state -eq 'done' -or $st.state -eq 'noop')) { $settled += "$($u.inputs.job.id)" }
                                elseif ($st -and $st.state -eq 'claimed' -and [int64]$st.token -eq [int64]$u.token) { $open += $u }
                            }
                        }
                    }
                    $units = $open
                }
            }
            Save 'units.json' $units
            if (-not $units.Count) { Save 'jobs.json' @(); Reply @{ ok = $true; jobs = 0; settled = $settled.Count; plan_revision = [int]$plan.json.revision } @{ units = @(); settled = $settled; plan = $plan.json }; break }
            $jobArgs += @('-Tokens', (($units | ForEach-Object { "$($_.inputs.job.id):$($_.token)" }) -join ','))
        } else {
            Save 'units.json' @()
        }
        $jobs = Run $jobArgs
        if (-not $jobs.json -or $jobs.json.ok -ne $true) { Fail 'jobs' $jobs.tail; break }
        Save 'jobs.json' @($jobs.json.jobs)
        $chunks = Save-Chunks 'jobs' @($jobs.json.jobs)
        Reply @{ ok = $true; jobs = @($jobs.json.jobs).Count; chunks = $chunks; settled = $settled.Count; plan_revision = [int]$plan.json.revision; file = (Join-Path $Drive 'jobs.json') } @{ units = $units; settled = $settled; plan = $plan.json }
    }
    'finish' {
        $check = Run @('cited-cards', '-Step', 'check', '-Staging', $Out)
        if (-not $check.json -or $check.json.ok -ne $true) { Fail 'check' $check.tail; break }
        $done = @(); $failed = @(); $late = @()
        $readiness = Join-Path $Out 'cited-cards\check\readiness.json'
        $heldUnits = @(Load 'units.json' | Where-Object { $_ -and $_.id })
        if ($heldUnits.Count) {
            $db = Run @('ledger', 'done', '-Out', $Out, '-UnitsFile', (Join-Path $Drive 'units.json'), '-Outputs', $readiness)
            $valid = Validate-DoneBatch $db $heldUnits
            if (-not $valid.ok) {
                $states = Reconcile-Units $heldUnits
                $stillClaimed = @()
                foreach ($u in $heldUnits) {
                    $st = $states["$($u.id)"]
                    if ($st -and ($st.state -eq 'done' -or $st.state -eq 'noop')) { $done += $u.inputs.job.id }
                    elseif ($st -and ($st.state -ne 'claimed' -or [int64]$st.token -ne [int64]$u.token)) { $late += $u }
                    else { $stillClaimed += $u }
                }
                if ($stillClaimed.Count) {
                    Fail 'done' "unreadable or incomplete batch done reply for $($stillClaimed.Count) unit(s): $($db.tail)" @{ done = $done; late = $late; remaining = $stillClaimed; raw = $db.tail }
                    break
                }
            } else {
                foreach ($u in $heldUnits) {
                    $r = $valid.map["$($u.id):$($u.token)"]
                    if ($r.status -eq 'done' -or $r.ledger_state -eq 'done') { $done += $u.inputs.job.id }
                    elseif ($r.status -eq 'stale_claim') { $late += $u }
                    else {
                        $cause = if ($r.cause) { "$($r.cause)" } else { "$($db.tail)" }
                        $failed += $u.inputs.job.id; Fail-Unit $u "the job $($u.inputs.job.id) is not ready: $cause"
                    }
                }
            }
        }
        # Reclaim expired claims (token-fenced), re-claim ONLY actually reclaimed IDs, and complete them in one batch.
        if ($late.Count) {
            Save 'late-units.json' $late
            $relLate = Run @('ledger', 'reclaim', '-Out', $Out, '-UnitsFile', (Join-Path $Drive 'late-units.json'))
            $reclaimedIds = @($relLate.json.reclaimed | Where-Object { $_ -and $_.unit } | ForEach-Object { "$($_.unit)" })
            $again = @{}
            $retryRes = @{}
            $db2Ok = $true
            if ($reclaimedIds.Count) {
                Save 'late-ids.json' $reclaimedIds
                $c = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'citedcards', '-IdsFile', (Join-Path $Drive 'late-ids.json'), '-All', '-Owner', 'cited cards finish', '-Lease', '3600')
                $reclaimed = @($c.json.units | Where-Object { $_ -and $_.id })
                foreach ($x in $reclaimed) { $again["$($x.id)"] = $x }
                if ($reclaimed.Count) {
                    Save 'late-reclaimed.json' $reclaimed
                    $db2 = Run @('ledger', 'done', '-Out', $Out, '-UnitsFile', (Join-Path $Drive 'late-reclaimed.json'), '-Outputs', $readiness)
                    $v2 = Validate-DoneBatch $db2 $reclaimed
                    $db2Ok = $v2.ok
                    if ($v2.ok) { $retryRes = $v2.map }
                    else {
                        $st2 = Reconcile-Units $reclaimed
                        foreach ($x in $reclaimed) {
                            $st = $st2["$($x.id)"]
                            if ($st -and ($st.state -eq 'done' -or $st.state -eq 'noop')) {
                                $retryRes["$($x.id):$($x.token)"] = [pscustomobject]@{ unit = $x.id; token = $x.token; status = 'done'; ledger_state = 'done' }
                            }
                        }
                    }
                }
            }
            foreach ($u in $late) {
                $x = $again["$($u.id)"]
                if (-not $x) { $failed += $u.inputs.job.id; continue }
                $r = $retryRes["$($x.id):$($x.token)"]
                if ($r -and ($r.status -eq 'done' -or $r.ledger_state -eq 'done')) { $done += $u.inputs.job.id }
                elseif (-not $db2Ok) {
                    Fail 'done' "unreadable late-recovery batch done reply for $($x.id)" @{ done = $done; failed = $failed; late = $late }
                    break
                } else {
                    $cause = if ($r -and $r.cause) { "$($r.cause)" } else { 'reclaimed job is not ready' }
                    $failed += $u.inputs.job.id; Fail-Unit $x "the job $($u.inputs.job.id) is not ready: $cause"
                }
            }
        }
        $bu = if ($HasLedger) { Claim-One 'citedcards' 'citedcards/book@' 'cited cards book' } else { $null }
        if ($bu) {
            $a = Run @('cited-cards', '-Step', 'apply', '-Staging', $Out, '-Token', "$($bu.token)", '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-PlanningStop 'apply' $aStop @{ done = $done; failed = $failed; refusal = $aStop.refusal }; break }
            if ($a.json -and $a.json.complete -eq $true) {
                $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $bu.id, '-Token', "$($bu.token)", '-Outputs', (Join-Path $Out 'cited-cards\report.json'))
                $bookState = if ($c.code -eq 0) { 'done' } else { 'failed' }
                if ($c.code -ne 0) { Fail-Unit $bu "the Cited Card book completion was refused: $($c.tail)" }
            } else { $bookState = 'failed'; Fail-Unit $bu "the publication is not complete: $($a.tail)" }
        } else {
            # A job that is not ready leaves the book unit blocked: the ready prefix is still published.
            $a = Run @('cited-cards', '-Step', 'apply', '-Staging', $Out, '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-PlanningStop 'apply' $aStop @{ done = $done; failed = $failed; refusal = $aStop.refusal }; break }
            # A resume finds the book unit already done in an earlier invocation (live 2026-11-13, bergin2026exchange):
            # nothing is claimable, and the ledger's own state answers.
            $bookState = if ($a.json -and $a.json.complete -eq $true -and -not $HasLedger) { 'done' } elseif ($HasLedger) {
                $s = Run @('ledger', 'status', '-Out', $Out, '-Layer', 'citedcards')
                $held = @(@($s.json.units) | Where-Object { $_ -and "$($_.id)" -like 'citedcards/book@*' }) | Select-Object -Last 1
                if ($held -and ($held.state -eq 'done' -or $held.state -eq 'noop')) { 'done' } else { 'incomplete' }
            } else { 'incomplete' }
        }
        $publishError = if (-not $a.json) { "$($a.tail)".Trim() } elseif ($a.json.ok -eq $false) { "$($a.json.error)".Trim() } else { '' }
        if ($publishError.Length -gt 160) { $publishError = $publishError.Substring($publishError.Length - 160) }
        Reply @{
            ok = $true; done = $done.Count; failed = $failed.Count; ready = [int]$check.json.ready; not_ready = [int]$check.json.not_ready
            book = $bookState; complete = ($a.json.complete -eq $true); rows = [int]$a.json.rows; committed = [int]$a.json.committed; publish_error = $publishError
        } @{ done = $done; failed = $failed; apply = $a.json; check = $check.json }
    }
}
