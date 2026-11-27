<#
.SYNOPSIS
  The digest layers' deterministic command chains, one step per call (#175, #180). A DeepSeek drive job runs
  one step and replies with its last line, so no Claude agent relays a single command.

  pwsh -NoProfile -File layer-steps.ps1 -Step start|apply|finish -Out <staging> -Layer <layer> -Owner <label> [options]

  start   claim every ready unit of the layer, run the layer's own start or plan command for them, and
          list the DeepSeek jobs it printed
                                                          -> layer-steps\<layer>\units.json, start.json, jobs.json
  apply   the second content round of the explain layer: apply the drafts and list the fix jobs it prints;
          of the terms layer: check every explorer answer and list the one merge job (`terms -Step merge`)
                                                          -> layer-steps\<layer>\apply-jobs.json
  finish  record every held unit's checked completion, fault the units that cannot finish, then run the
          layer's own apply and report commands and its ledger gate

  The Cards layer's work is the checked finish alone: its `start` claims and finishes every ready
  check, page by page, and has no finish step.

  #180: each step prints ONE compact JSON line: `ok` first, then the counts and file paths the module reads
  (units, jobs, file, gate_complete, gate_open, gate_total, done, failed), at most 400 characters. The gate,
  the per-unit results and the layer's apply/report lines go to <staging>\layer-steps\<layer>\drive-lines\
  <step>.json, named in the line as `detail`. A module reads no unit list and no nested object from a line.

  A unit whose completion the owner's Lock refused keeps that refusal as its content fault and stops the
  step: it prints {ok:false, code:"card_locked", message, tree, lock_id, unit, error, detail} and the module
  halts the stage with it.
#>
param(
    [Parameter(Mandatory)][ValidateSet('start', 'apply', 'finish')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [Parameter(Mandatory)][ValidateSet('digest', 'link', 'cards', 'events', 'findings', 'explain', 'terms')][string]$Layer,
    [string]$Owner = '',
    [ValidateRange(0, 7200)][int]$Lease = 0,
    [ValidateRange(0, 1000)][int]$MaxParts = 0,
    [string]$Limits = '',
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'step-line.ps1')
$Out = $Out.TrimEnd('\', '/')
$Work = Join-Path (Join-Path $Out 'layer-steps') $Layer
New-Item -ItemType Directory -Force $Work | Out-Null
if (-not $Owner) { $Owner = "$Layer steps" }
$LineName = $Step
# One page of a claim: the claim commands hold at most eight units, and a start repeats them until none is ready.
$Page = 8

# One command: its last JSON line (or $null), its exit code and its last lines.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = ((@($text) | Select-Object -Last 3) -join ' | ') }
}
function Save($Name, $Value) {
    $list = @($Value)
    # -InputObject keeps a one-entry list an array: piped, ConvertTo-Json writes a bare object, and a manager
    # told to read a JSON array then runs nothing (test box run 2026-11-01).
    $text = if ($list.Count) { ConvertTo-Json -InputObject $list -Depth 64 } else { '[]' }
    Set-Content -LiteralPath (Join-Path $Work $Name) -Value $text -Encoding utf8
}
function Load($Name) { $p = Join-Path $Work $Name; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64 } }
# The step's answer: its compact line and the detail file beside it (#180).
function Reply($Value, $Detail = $null) { Send-StepLine -Line $Value -Detail $Detail -Name $LineName -Out $Work }
function Fail($Step, $Reason, $Detail = $null, $Extra = $null) { Send-StepFail -Step $Step -Reason $Reason -Name $LineName -Out $Work -Detail $Detail -Extra $Extra }
function Tail([string]$Text) {
    $t = "$Text".Trim()
    if ($t.Length -gt 400) { return $t.Substring(0, 400) }
    return $t
}
function Gate {
    $g = Run @('ledger', 'gate', '-Out', $Out, '-Layer', $Layer, '-Brief')
    if ($g.json) { return $g.json }
    return @{ ok = $false; complete = $false; error = (Tail $g.tail) }
}
# The gate's three counts as the line's own flat fields (#180): the module reads no nested gate.
function Gate-Fields($Gate) {
    return @{
        gate_complete = ($Gate -and $Gate.complete -eq $true)
        gate_open = [int]$(if ($Gate -and $Gate.counts) { $Gate.counts.open } else { 0 })
        gate_total = [int]$(if ($Gate -and $Gate.counts) { $Gate.counts.total } else { 0 })
    }
}
function Group-Of([string]$Unit) {
    $m = [regex]::Match("$Unit", 'g(\d+)@')
    if ($m.Success) { return [int]$m.Groups[1].Value }
    return 0
}
# One unit's fault, classified by the ledger (a code only when the fault is a content refusal).
function Fail-Unit($Unit, [string]$Cause, [string]$Code = '') {
    $fault = @{ schema_version = 1; unit = $Unit.id; input_hash = "$($Unit.input_hash)"; cause = (Tail $Cause) }
    if ($Code) { $fault['code'] = $Code }
    $text = $fault | ConvertTo-Json -Compress
    $out = Run @('ledger', 'fail', '-Out', $Out, '-Unit', "$($Unit.id)", '-Token', "$($Unit.token)", '-Fault', $text)
    if ($out.json -and $out.json.state -eq 'failed') { return 'failed' }
    if ($out.json) { return 'retry' }
    if ($Code -eq 'check_failed' -or $Code -eq 'card_locked') { return 'failed' }
    return 'retry'
}
# The owner's typed planning-stop refusal of an answer (`card_locked` or `scope_refused` JSON).
function Lock-Refusal([string]$Text) {
    foreach ($line in "$Text".Split("`n")) {
        $t = $line.Trim()
        if (-not $t.StartsWith('{')) { continue }
        try { $v = $t | ConvertFrom-Json -Depth 16 } catch { continue }
        if ("$($v.code)" -in @('card_locked', 'scope_refused') -or "$($v.error)" -in @('card_locked', 'scope_refused')) { return $v }
    }
    return $null
}
function Limits-Args {
    if (-not $Limits) { return @() }
    return @($Limits -split '\s+' | Where-Object { $_ })
}
# The units a claim still holds when its line is unreadable: the tool's own ledger read by owner.
function Held-Units {
    $h = Run @('ledger', 'status', '-Out', $Out, '-Held', $Owner)
    if ($h.json -and $h.json.units -is [array]) { return @($h.json.units | Where-Object { $_ -and $_.id }) }
    return @()
}
# The ledger's own state of the named units (the answer to a cut completion line).
function Unit-States([string]$List) {
    $s = Run @('ledger', 'status', '-Out', $Out, '-Units', $List)
    if ($s.json -and $s.json.units -is [array]) { return @($s.json.units) }
    return @()
}
function Finished($Json) { return ($Json -and ($Json.state -eq 'done' -or $Json.complete -eq $true)) }
# The jobs one start or plan line names inline, or in the job file its "job" path names.
function Jobs-Of($Json) {
    if (-not $Json) { return @() }
    if ($null -ne $Json.jobs) { return @($Json.jobs) }
    if ($Json.job) {
        $path = "$($Json.job)"
        if (Test-Path -LiteralPath $path) {
            $doc = Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64
            if ($null -ne $doc.jobs) { return @($doc.jobs) }
        }
    }
    return @()
}
# docs/digest-job-size-proposal.md: one Link job holds up to three groups and up to 30 gaps, one conversation for several
# drafts, so a large group runs alone. Each group keeps its own unit, draft, SELF-CHECK and link-finish. The conversation is
# recorded once, under its first group's unit.
function Join-LinkJobs($Jobs) {
    $joined = @()
    $part = @()
    $gaps = 0
    $flush = {
        if ($part.Count -eq 1) { $script:joinedOut += $part[0] }
        elseif ($part.Count -gt 1) {
            $first = $part[0]
            $drafts = (@($part | ForEach-Object { $_.output }) -join '、')
            $name = ($first.id -replace '-t[0-9]+$', '') + ($part[-1].id -replace '^link(-g[0-9]+)-t[0-9]+$', '$1') + "-t$($first.token)"
            $script:joinedOut += [pscustomobject]@{
                id = $name; kind = $first.kind; files = @($part | ForEach-Object { $_.files } | Select-Object -Unique); output = $first.output; permissions = $first.permissions
                unit = $first.unit; token = $first.token
                prompt = "本任务含 $($part.Count) 组，每组有自己的任务文件。按任务说明把每组的连接草稿写入它自己的文件：$drafts。每份草稿保留本组的绑定字段，并运行本组的 SELF-CHECK 行。"
            }
        }
    }
    $script:joinedOut = @()
    foreach ($j in @($Jobs)) {
        $count = [int]$j.gaps
        if ($part.Count -and ($part.Count -ge 3 -or ($gaps + $count) -gt 30)) { & $flush; $part = @(); $gaps = 0 }
        $part += $j
        $gaps += $count
    }
    & $flush
    return $script:joinedOut
}
# The layer's own start or plan command for every claimed unit, and the jobs it lists.
function Start-Layer($Units) {
    $firstUnit = $Units[0]
    if ($Layer -eq 'digest' -or $Layer -eq 'link') {
        $startName = if ($Layer -eq 'digest') { 'group-start' } else { 'link-start' }
        $unitsPath = Join-Path $Work 'units.json'
        $b = Run @($startName, '-Out', $Out, '-UnitsFile', $unitsPath)
        if (-not $b.json -or $b.json.kind -ne ("{0}_start_batch" -f $(if ($Layer -eq 'digest') { 'group' } else { 'link' }))) {
            return @([pscustomobject]@{ unit = $firstUnit; group = (Group-Of $firstUnit.id); line = $b; jobs = @(); error = (Tail $b.tail) })
        }
        $byUnit = @{}
        if ($b.json.file -and (Test-Path -LiteralPath "$($b.json.file)")) {
            $full = Get-Content -LiteralPath "$($b.json.file)" -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64
            foreach ($rec in @($full.units | Where-Object { $_ -and $_.unit })) { $byUnit["$($rec.unit)"] = $rec }
        }
        $outList = @()
        foreach ($r in @($b.json.results | Where-Object { $_ })) {
            $u = @($Units | Where-Object { $_.id -eq $r.unit }) | Select-Object -First 1
            if (-not $u) { $u = [pscustomobject]@{ id = $r.unit; token = $r.token; input_hash = "$($r.input_hash)" } }
            $rec = $byUnit["$($r.unit)"]
            $detail = if ($rec -and $rec.result) { $rec.result } else { $r }
            if ($r.status -ne 'started') {
                $errMsg = if ($r.error) { "$($r.error)" } elseif ($rec -and $rec.error) { "$($rec.error)" } else { "start status $($r.status)" }
                $outList += [pscustomobject]@{ unit = $u; group = (Group-Of $u.id); line = [pscustomobject]@{ code = 1; json = $null; tail = $errMsg }; jobs = @(); error = (Tail $errMsg) }
                continue
            }
            $outList += [pscustomobject]@{ unit = $u; group = (Group-Of $u.id); line = [pscustomobject]@{ code = 0; json = $detail; tail = '' }; jobs = (Jobs-Of $detail); error = $null }
        }
        return $outList
    }
    $one = switch ($Layer) {
        'events' { Run @('event', '-Step', 'plan', '-Staging', $Out) }
        'findings' { Run @('finding', '-Step', 'plan', '-Staging', $Out) }
        'explain' {
            $models = (@($Units | ForEach-Object { $_.inputs.model } | Where-Object { $_ }) -join ',')
            $argv = @('model', '-Step', 'explain', '-Work', (Join-Path $Out 'explain'), '-Staging', $Out) + (Limits-Args) + @('-Models', $models)
            if ($MaxParts -gt 0) { $argv += @('-MaxParts', "$MaxParts") }
            Run $argv
        }
        'terms' { Run @('terms', '-Step', 'plan', '-Staging', $Out, '-Token', "$($firstUnit.token)") }
    }
    [pscustomobject]@{ unit = $firstUnit; group = (Group-Of $firstUnit.id); line = $one; jobs = (Jobs-Of $one.json); error = $(if ($one.json) { $null } else { (Tail $one.tail) }) }
}
# Check a native batch or step result for a typed planning-stop refusal (`card_locked` or `scope_refused`)
# BEFORE any report, aggregation, continuation, or recovery. Never classifies an untyped exit 2 as a Lock.
function Batch-LockRefusal($RunResult) {
    if ($RunResult.json) {
        $code = ''
        if ("$($RunResult.json.code)" -in @('card_locked', 'scope_refused')) { $code = "$($RunResult.json.code)" }
        elseif ("$($RunResult.json.error)" -in @('card_locked', 'scope_refused')) { $code = "$($RunResult.json.error)" }
        if ($code) {
            $ref = if ($RunResult.json.refusal) { $RunResult.json.refusal } else { $RunResult.json }
            $lockedUnit = @($RunResult.json.results | Where-Object { $_ -and "$($_.code)" -in @('card_locked', 'scope_refused') }) | Select-Object -First 1
            if (-not $lockedUnit -and $RunResult.json.explain) {
                $lockedUnit = @($RunResult.json.explain | Where-Object { $_ -and "$($_.code)" -in @('card_locked', 'scope_refused') }) | Select-Object -First 1
            }
            return @{
                code = $code
                unit = if ($lockedUnit) { "$($lockedUnit.unit)" } else { '' }
                message = if ($ref.message) { "$($ref.message)" } elseif ($RunResult.json.message) { "$($RunResult.json.message)" } else { $code }
                tree = "$($ref.tree)"
                lock_id = $ref.lock_id
                refusal = $ref
            }
        }
    }
    $found = Lock-Refusal "$($RunResult.tail)"
    if ($found) {
        $code = if ("$($found.code)" -in @('card_locked', 'scope_refused')) { "$($found.code)" } else { "$($found.error)" }
        return @{
            code = $code
            unit = ''
            message = if ($found.message) { "$($found.message)" } else { $code }
            tree = "$($found.tree)"
            lock_id = $found.lock_id
            refusal = $found
        }
    }
    return $null
}
function Fail-PlanningStop([string]$StepName, $StopObj, $Detail = $null) {
    $code = if ($StopObj.code) { "$($StopObj.code)" } else { 'card_locked' }
    Fail $StepName "${code}: $($StopObj.message)" $Detail @{ code = $code; layer = $Layer; unit = "$($StopObj.unit)"; message = "$($StopObj.message)"; tree = "$($StopObj.tree)"; lock_id = $StopObj.lock_id }
}
# Read persisted `remaining` pairs from a batch detail file for budget continuation; returns $null if missing/unreadable.
function Read-Remaining($BatchJson) {
    if (-not $BatchJson -or -not $BatchJson.file -or -not (Test-Path -LiteralPath "$($BatchJson.file)")) { return $null }
    try {
        $full = Get-Content -LiteralPath "$($BatchJson.file)" -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64
        return @($full.remaining | Where-Object { $_ -and $_.id -and $_.token })
    } catch { return $null }
}
# The Link pass's own totals (#180): the run report carries the gap counts and the L1/L2 quality of the whole
# layer, summed from the units' completions, never the per-unit rows.
function Link-Line($Results) {
    $list = @($Results | Where-Object { $_ })
    $fields = @{}
    foreach ($name in @('gaps', 'skipped', 'linked', 'linked_from', 'already_linked', 'no_link')) {
        $total = 0
        foreach ($r in $list) { if ($r.counts -and $null -ne $r.counts.$name) { $total += [int]$r.counts.$name } }
        if ($total) { $fields["link_$name"] = $total }
    }
    $changed = 0
    foreach ($r in $list) { $changed += @($r.changed).Count }
    if ($changed) { $fields.link_changed = $changed }
    $scheduled = @($list | Where-Object { [int]$_.jobs -gt 0 }).Count
    if ($scheduled) { $fields.link_scheduled = $scheduled }
    $faults = 0; $checked = 0
    foreach ($r in $list) { if ($r.quality) { $faults += [int]$r.quality.faults; $checked += [int]$r.quality.checked } }
    if ($faults) { $fields.link_quality_faults = $faults }
    if ($checked) { $fields.link_quality_checked = $checked }
    return $fields
}
# The compact fields of a finish answer (#180): the module reads counts, never the per-unit results. A zero is
# left out: the module reads an absent count as zero, and the line stays short.
function Results-Line($Results) {
    $list = @($Results | Where-Object { $_ })
    $fields = @{}
    $done = @($list | Where-Object { $_.status -eq 'done' -or $_.status -eq 'partial' }).Count
    $failed = @($list | Where-Object { $_.status -ne 'done' -and $_.status -ne 'partial' }).Count
    $partial = @($list | Where-Object { $_.status -eq 'partial' }).Count
    if ($done) { $fields.done = $done }
    if ($failed) { $fields.failed = $failed }
    if ($partial) { $fields.partial = $partial }
    return $fields
}

$units = @()
$results = @()

switch ($Step) {
    'start' {
        if ($Layer -eq 'cards') {
            # The Cards layer's work is the checked finish alone: drain every ready check in bounded
            # internal chunks, resuming any claimed `remaining` tail with `-UnitsFile` before claiming
            # fresh work, and stopping immediately on a typed planning stop or transport/archive fault.
            $argv = @('finish-batch', '-Out', $Out, '-Layer', 'cards', '-All', '-Owner', $Owner)
            if ($Lease -gt 0) { $argv += @('-Lease', "$Lease") }
            $all = @(); $claimed = 0; $lock = $null; $refused = $null; $fatalStop = $null
            $remRound = 0
            while ($true) {
                $line = Run $argv
                $lock = Batch-LockRefusal $line
                if ($lock) {
                    if ($line.json -and $line.json.kind -eq 'finish_batch') {
                        $all += @($line.json.results | Where-Object { $_ -and $_.status -ne 'not_run' })
                    }
                    break
                }
                if (-not $line.json -or $line.json.kind -ne 'finish_batch') { $refused = $line; break }
                $all += @($line.json.results | Where-Object { $_ -and $_.status -ne 'not_run' })
                $claimed += [int]$line.json.claimed
                $stop = "$($line.json.stop_reason)"
                $notRun = [int]$line.json.not_run
                if ($stop -eq 'transport' -or $stop -eq 'archive_identity') {
                    $fatalStop = "${stop}: $notRun cards unit(s) not run"
                    break
                }
                if ($stop -ne 'budget') {
                    if ($notRun -gt 0) { $fatalStop = "incomplete_batch: $notRun cards unit(s) not run" }
                    break
                }
                # Budget expired: first drain any exact claimed tail in `remaining` via `-UnitsFile`.
                if ($notRun -gt 0) {
                    $pending = Read-Remaining $line.json
                    if ($null -eq $pending -or $pending.Count -eq 0) {
                        $fatalStop = "budget_continuation: missing remaining detail for $notRun claimed cards unit(s)"
                        break
                    }
                    while ($pending.Count) {
                        $remRound++
                        $remName = "cards-rem-$remRound.json"
                        Save $remName $pending
                        $cont = Run @('finish-batch', '-Out', $Out, '-UnitsFile', (Join-Path $Work $remName))
                        $lock = Batch-LockRefusal $cont
                        if ($cont.json -and $cont.json.kind -eq 'finish_batch') {
                            $all += @($cont.json.results | Where-Object { $_ -and $_.status -ne 'not_run' })
                        }
                        if ($lock) { break }
                        if (-not $cont.json -or $cont.json.kind -ne 'finish_batch') {
                            $refused = $cont
                            break
                        }
                        $cStop = "$($cont.json.stop_reason)"
                        $cNotRun = [int]$cont.json.not_run
                        if ($cStop -eq 'transport' -or $cStop -eq 'archive_identity') {
                            $fatalStop = "${cStop}: $cNotRun cards unit(s) not run"
                            break
                        }
                        if ($cStop -eq 'budget' -or $cNotRun -gt 0) {
                            $nextPending = Read-Remaining $cont.json
                            if ($null -eq $nextPending -or $nextPending.Count -eq 0) {
                                $fatalStop = "budget_continuation: missing remaining detail for $cNotRun cards unit(s)"
                                break
                            }
                            if ($nextPending.Count -ge $pending.Count) {
                                $fatalStop = "budget_continuation: remaining set did not shrink ($($nextPending.Count) >= $($pending.Count))"
                                break
                            }
                            $pending = $nextPending
                            continue
                        }
                        $pending = @()
                    }
                    if ($lock -or $refused -or $fatalStop) { break }
                    continue
                }
                # Budget expired with zero claimed in this chunk: do not infinite-loop.
                if ([int]$line.json.claimed -eq 0) {
                    $fatalStop = 'budget: admission budget expired before claiming ready cards units'
                    break
                }
            }
            Save 'units.json' @()
            $gate = Gate
            if ($lock) {
                Fail-PlanningStop 'finish' $lock @{ results = $all; gate = $gate; refusal = $lock.refusal }
                break
            }
            if ($fatalStop) {
                Fail 'finish' $fatalStop @{ results = $all; gate = $gate }
                break
            }
            if ($refused) {
                # The line came back unreadable: the ledger decides. Finish whatever this owner still holds;
                # a non-zero refusal without held units fails the step.
                $held = @(Held-Units)
                if ($held.Count) {
                    Save 'units-recover.json' $held
                    $again = Run @('finish-batch', '-Out', $Out, '-UnitsFile', (Join-Path $Work 'units-recover.json'))
                    $againLock = Batch-LockRefusal $again
                    $gate = Gate
                    if ($againLock) {
                        Fail-PlanningStop 'finish' $againLock @{ results = @($again.json.results); gate = $gate; refusal = $againLock.refusal }
                        break
                    }
                    if (-not $again.json -or $again.json.kind -ne 'finish_batch') { Fail 'finish' (Tail $again.tail) @{ gate = $gate }; break }
                    if ([int]$again.json.not_run -gt 0 -or "$($again.json.stop_reason)" -in @('transport', 'archive_identity', 'budget')) {
                        Fail 'finish' "$($again.json.stop_reason): $([int]$again.json.not_run) recovered cards unit(s) not run" @{ results = @($again.json.results); gate = $gate }
                        break
                    }
                    $againResults = @($again.json.results)
                    $fields = Results-Line $againResults
                    Reply (@{ ok = $true; layer = 'cards'; claimed = $held.Count; units = $held.Count; jobs = 0 } + $fields + (Gate-Fields $gate)) @{ results = $againResults; gate = $gate; units = @() }
                    break
                }
                if ($refused.code -ne 0) { Fail 'cards claim' (Tail $refused.tail); break }
                Reply (@{ ok = $true; layer = 'cards'; claimed = 0; units = 0; jobs = 0; results = 0; done = 0; failed = 0; partial = 0 } + (Gate-Fields $gate)) @{ results = @(); gate = $gate; units = @() }
                break
            }
            $fields = Results-Line $all
            Reply (@{ ok = $true; layer = 'cards'; claimed = $claimed; units = $claimed; jobs = 0 } + $fields + (Gate-Fields $gate)) @{ results = $all; gate = $gate; units = @() }
        }
        # A start begins the layer, so any claim still held belongs to a stopped run: release every
        # claimed unit of this layer in one command, carrying only the first pass's exact (id, token)
        # pairs through quiet-window retries so later passes never touch newly acquired claims.
        $wait = if ($env:ZT_RELEASE_WAIT_SECONDS) { [int]$env:ZT_RELEASE_WAIT_SECONDS } else { 125 }
        $refusal = $null
        $retryTargets = $null
        for ($w = 0; $w -le 5; $w++) {
            if ($null -eq $retryTargets) {
                $rel = Run @('ledger', 'reclaim', '-Out', $Out, '-Layer', $Layer, '-All', '-Release')
            } else {
                Save 'release-retry.json' $retryTargets
                $rel = Run @('ledger', 'reclaim', '-Out', $Out, '-UnitsFile', (Join-Path $Work 'release-retry.json'), '-Release')
            }
            if ($rel.code -ne 0 -or -not $rel.json) {
                if ("$($rel.tail)" -match 'claim_recent' -and $w -lt 5) {
                    Start-Sleep -Seconds $wait
                    continue
                }
                $refusal = Tail $rel.tail
                break
            }
            $blocked = @($rel.json.blocked | Where-Object { $_ })
            $skipped = @($rel.json.skipped | Where-Object { $_ })
            $recent = @($skipped | Where-Object { $_.reason -eq 'claim_recent' })
            $otherSkipped = @($skipped | Where-Object { $_.reason -ne 'claim_recent' })
            if ($blocked.Count) {
                $refusal = "$($blocked[0].unit): $($blocked[0].reason)"
                break
            }
            if ($otherSkipped.Count) {
                # When retrying an explicit fenced set, `claim_active` means the claim moved on / was settled.
                $realOther = if ($null -ne $retryTargets) { @($otherSkipped | Where-Object { $_.reason -ne 'claim_active' }) } else { $otherSkipped }
                if ($realOther.Count) {
                    $refusal = "$($realOther[0].unit): $($realOther[0].reason)"
                    break
                }
            }
            if (-not $recent.Count) { break }
            if ($w -eq 5) {
                $refusal = "$($recent[0].unit): claim_recent"
                break
            }
            # Freeze the exact (id, token) pairs from the first pass for quiet-window retries.
            if ($null -eq $retryTargets) {
                $withTokens = @($recent | Where-Object { $_.unit -and $_.token } | ForEach-Object { @{ id = "$($_.unit)"; token = [int64]$_.token } })
                if ($withTokens.Count -eq $recent.Count) {
                    $retryTargets = $withTokens
                } else {
                    $ids = (@($recent | ForEach-Object { "$($_.unit)" }) -join ',')
                    $st = @(Unit-States $ids)
                    $retryTargets = @($st | Where-Object { $_.state -eq 'claimed' -and $_.id -and $_.token } | ForEach-Object { @{ id = "$($_.id)"; token = [int64]$_.token } })
                    if (-not $retryTargets.Count) { break }
                }
            }
            Start-Sleep -Seconds $wait
        }
        if ($refusal) { Fail 'release' $refusal; break }
        $claimArgs = @('ledger', 'next', '-Out', $Out, '-Layer', $Layer, '-All', '-Brief', '-Owner', $Owner)
        if ($Lease -gt 0) { $claimArgs += @('-Lease', "$Lease") }
        # A claim that fails is tried up to three times; after any ambiguous/failed call, reconcile
        # this owner's held claims immediately before retrying so a committed-then-lost reply is
        # never masked by a subsequent empty claim.
        $claim = $null
        $units = @()
        $readable = $false
        for ($try = 1; $try -le 3; $try++) {
            $claim = Run $claimArgs
            if ($claim.json -and $claim.json.units -is [array]) {
                $units = @($claim.json.units | Where-Object { $_ -and $_.id })
                $readable = $true
                break
            }
            $held = @(Held-Units)
            if ($held.Count) {
                $units = $held
                $readable = $true
                break
            }
            if ($claim.code -eq 0) { break }
        }
        if (-not $readable) {
            $units = @(Held-Units)
            if (-not $units.Count -and -not $claim.json) { Fail 'claim' (Tail $claim.tail); break }
        }
        Save 'units.json' $units
        if (-not $units.Count) {
            $gate = Gate
            Reply (@{ ok = $true; layer = $Layer; units = 0; claimed = 0; jobs = 0 } + (Gate-Fields $gate)) @{ units = @(); gate = $gate; plan = $null }
        }

        $begun = @(Start-Layer $units)
        foreach ($b in $begun) {
            $stLock = Batch-LockRefusal $b.line
            if ($stLock) {
                $gate = Gate
                Fail-PlanningStop "$Layer start" $stLock @{ units = $units; gate = $gate; refusal = $stLock.refusal }
                break
            }
        }
        $bad = @($begun | Where-Object { $_.error })
        if ($bad.Count) { Fail "$Layer start" (Tail $bad[0].error) @{ units = $units }; break }
        $starts = @()
        $allJobs = @()
        foreach ($b in $begun) {
            $u = $b.unit
            $starts += [pscustomobject]@{
                unit = $u.id; token = $u.token; group = $b.group; input_hash = "$($u.input_hash)";
                draft = "$($b.line.json.draft)"; expected_jobs = [int]$(if ($b.line.json.expected_jobs) { $b.line.json.expected_jobs } else { @($b.jobs).Count });
                recovered = [bool]$b.line.json.recovered; jobs = @($b.jobs).Count
            }
            foreach ($j in @($b.jobs)) {
                $j | Add-Member -NotePropertyName unit -NotePropertyValue $u.id -Force
                $j | Add-Member -NotePropertyName token -NotePropertyValue $u.token -Force
                $allJobs += $j
            }
        }
        Save 'start.json' $starts
        if ($Layer -eq 'link') { $allJobs = @(Join-LinkJobs $allJobs) }
        Save 'jobs.json' $allJobs
        # One job file per manager: a manager runs every entry of its own file (zt-term-cards v11).
        Get-ChildItem -LiteralPath $Work -Filter 'jobs-*.json' -ErrorAction SilentlyContinue | Remove-Item -Force
        $chunks = [int][Math]::Ceiling(@($allJobs).Count / 8)
        for ($k = 0; $k -lt $chunks; $k++) { Save ('jobs-{0:D3}.json' -f ($k + 1)) @($allJobs | Select-Object -Skip ($k * 8) -First 8) }
        $plan = $null
        if ($Layer -eq 'events' -or $Layer -eq 'findings') { $plan = @{ candidates = [int]$begun[0].line.json.candidates; groups = [int]$begun[0].line.json.groups; jobs = @($begun[0].jobs).Count } }
        if ($Layer -eq 'explain') { $plan = @{ parts = [int]$begun[0].line.json.parts; jobs = @($begun[0].jobs).Count } }
        $gate = Gate
        $line = @{ ok = $true; layer = $Layer; units = $units.Count; jobs = @($allJobs).Count; chunks = $chunks; file = (Join-Path $Work 'jobs.json') }
        if ($plan -and $plan.ContainsKey('candidates')) { $line.plan_candidates = [int]$plan.candidates; $line.plan_groups = [int]$plan.groups }
        if ($plan -and $plan.ContainsKey('parts')) { $line.plan_parts = [int]$plan.parts }
        Reply ($line + (Gate-Fields $gate)) @{ units = $units; starts = $starts; jobs = $allJobs; plan = $plan; gate = $gate }
    }
    'apply' {
        if ($Layer -ne 'explain' -and $Layer -ne 'terms') { Fail 'apply' "the $Layer layer has no apply step"; break }
        $units = @(Load 'units.json')
        if ($Layer -eq 'terms' -and $units.Count) {
            # The aggregation round: the explorers' answers are checked and one merge job is listed. A missing
            # or unbound explorer answer stops the stage; the finish step then faults the unit.
            $u = $units[0]
            $r = Run @('terms', '-Step', 'merge', '-Staging', $Out, '-Token', "$($u.token)")
            $rLock = Batch-LockRefusal $r
            if ($rLock) { Fail-PlanningStop 'terms merge' $rLock @{ units = $units; refusal = $rLock.refusal }; break }
            if (-not $r.json) { Fail 'terms merge' (Tail $r.tail) @{ units = $units }; break }
            $jobs = @()
            foreach ($j in @(Jobs-Of $r.json)) {
                $j | Add-Member -NotePropertyName unit -NotePropertyValue $u.id -Force
                $j | Add-Member -NotePropertyName token -NotePropertyValue $u.token -Force
                $jobs += $j
            }
            Save 'apply-jobs.json' $jobs
            $gate = Gate
            Reply (@{ ok = $true; layer = $Layer; jobs = @($jobs).Count; parts = [int]$r.json.parts; rows = [int]$r.json.rows; distinct = [int]$r.json.distinct; file = (Join-Path $Work 'apply-jobs.json') } + (Gate-Fields $gate)) @{ jobs = $jobs; gate = $gate; merge = $r.json }
        }
        if (-not $units.Count) {
            $gate = Gate
            Reply (@{ ok = $true; layer = $Layer; jobs = 0; applied = 0; refused = 0; blocked = 0 } + (Gate-Fields $gate)) @{ jobs = @(); gate = $gate }
        }
        $argv = @('model', '-Step', 'explain-apply', '-Work', (Join-Path $Out 'explain'), '-Staging', $Out) + (Limits-Args)
        $r = Run $argv
        $rLock = Batch-LockRefusal $r
        if ($rLock) { Fail-PlanningStop 'explain-apply' $rLock @{ units = $units; refusal = $rLock.refusal }; break }
        if (-not $r.json) { Fail 'explain-apply' (Tail $r.tail) @{ units = $units }; break }
        $firstUnit = $units[0]
        $jobs = @()
        foreach ($j in @(Jobs-Of $r.json)) {
            $j | Add-Member -NotePropertyName unit -NotePropertyValue $firstUnit.id -Force
            $j | Add-Member -NotePropertyName token -NotePropertyValue $firstUnit.token -Force
            $jobs += $j
        }
        Save 'apply-jobs.json' $jobs
        $gate = Gate
        Reply (@{ ok = $true; layer = $Layer; jobs = @($jobs).Count; applied = [int]$r.json.applied; refused = (Count-Of $r.json.refused); blocked = (Count-Of $r.json.blocked); file = (Join-Path $Work 'apply-jobs.json') } + (Gate-Fields $gate)) @{ jobs = $jobs; gate = $gate; apply = $r.json }
    }
    'finish' {
        $units = @(Load 'units.json')
        if ($Layer -eq 'cards' -or -not $units.Count) {
            $gate = Gate
            Reply (@{ ok = $true; layer = $Layer; results = 0; done = 0; failed = 0; partial = 0 } + (Gate-Fields $gate)) @{ results = @(); gate = $gate; mid = @() }
        }

        if ($Layer -eq 'digest' -or $Layer -eq 'link') {
            $starts = @(Load 'start.json')
            $finishCmd = if ($Layer -eq 'digest') { 'group-finish' } else { 'link-finish' }
            $expectedKind = if ($Layer -eq 'digest') { 'digest_finish_batch' } else { 'link_finish_batch' }
            $pending = @($units)
            $lock = $null
            $fatalStop = $null
            $round = 0
            while ($pending.Count) {
                $round++
                $uFile = if ($round -eq 1) { Join-Path $Work 'units.json' } else { $n = "units-rem-$round.json"; Save $n $pending; Join-Path $Work $n }
                $b = Run @($finishCmd, '-Out', $Out, '-UnitsFile', $uFile)
                $lock = Batch-LockRefusal $b
                if ($b.json -and $b.json.kind -eq $expectedKind) {
                    foreach ($r in @($b.json.results | Where-Object { $_ -and $_.status -ne 'not_run' })) {
                        $s = @($starts | Where-Object { $_.unit -eq $r.unit }) | Select-Object -First 1
                        $entry = @{ unit = "$($r.unit)"; token = $r.token; group = (Group-Of "$($r.unit)"); status = "$($r.status)"; jobs = $(if ($s) { [int]$s.expected_jobs } else { 0 }) }
                        foreach ($k in @('disposition', 'owned', 'recovered', 'counts', 'quality', 'changed', 'code', 'cause', 'from')) {
                            if ($null -ne $r.$k) { $entry[$k] = $r.$k }
                        }
                        $results += $entry
                    }
                }
                if ($lock) { break }
                if (-not $b.json -or $b.json.kind -ne $expectedKind) {
                    # An unreadable batch line asks the ledger before any fault.
                    $states = @(Unit-States ((@($pending | ForEach-Object { $_.id })) -join ','))
                    foreach ($u in $pending) {
                        $s = @($starts | Where-Object { $_.unit -eq $u.id }) | Select-Object -First 1
                        $said = @($states | Where-Object { $_.id -eq $u.id }) | Select-Object -First 1
                        if ($said -and ($said.state -eq 'done' -or $said.state -eq 'noop')) {
                            $results += @{ unit = $u.id; token = $u.token; group = (Group-Of $u.id); status = 'done'; from = 'ledger'; jobs = $(if ($s) { [int]$s.expected_jobs } else { 0 }) }
                            continue
                        }
                        $cause = "missing_result: the completion line was not readable: $(Tail $b.tail)"
                        $status = Fail-Unit $u $cause
                        $results += @{ unit = $u.id; token = $u.token; group = (Group-Of $u.id); status = $status; cause = (Tail $cause) }
                    }
                    break
                }
                $stop = "$($b.json.stop_reason)"
                $notRun = [int]$b.json.not_run
                if ($stop -eq 'transport' -or $stop -eq 'archive_identity') {
                    $fatalStop = "${stop}: $notRun $Layer unit(s) not run"
                    break
                }
                if ($stop -eq 'budget' -or $notRun -gt 0) {
                    $nextPending = Read-Remaining $b.json
                    if ($null -eq $nextPending -or $nextPending.Count -eq 0) {
                        $fatalStop = "budget_continuation: missing remaining detail for $notRun $Layer unit(s)"
                        break
                    }
                    if ($nextPending.Count -ge $pending.Count) {
                        $fatalStop = "budget_continuation: remaining set did not shrink ($($nextPending.Count) >= $($pending.Count))"
                        break
                    }
                    $pending = $nextPending
                    continue
                }
                break
            }
            $gate = Gate
            if ($lock) {
                Fail-PlanningStop 'finish' $lock @{ results = $results; gate = $gate; refusal = $lock.refusal }
                break
            }
            if ($fatalStop) {
                Fail 'finish' $fatalStop @{ results = $results; gate = $gate }
                break
            }
            $linkLine = if ($Layer -eq 'link') { Link-Line $results } else { @{} }
            Reply (@{ ok = $true; layer = $Layer } + (Results-Line $results) + $linkLine + (Gate-Fields $gate)) @{ results = $results; gate = $gate; mid = @() }
        }

        # The layers whose completions finish-batch runs: apply and report first (stopping immediately
        # on any typed Lock/scope refusal BEFORE report or finish-batch), then every held unit's
        # checked completion in one command.
        $mid = @()
        switch ($Layer) {
            'events' {
                $a = Run @('event', '-Step', 'apply', '-Staging', $Out, '-Apply')
                $aLock = Batch-LockRefusal $a
                if ($aLock) {
                    $gate = Gate
                    Fail-PlanningStop 'apply' $aLock @{ mid = $mid; gate = $gate; refusal = $aLock.refusal }
                    break
                }
                $mid += @{ step = 'apply'; ok = [bool]$a.json; applied = [int]$a.json.applied_jobs; created = [int]$a.json.created; merged = [int]$a.json.merged; linked = [int]$a.json.linked; refused = (Count-Of $a.json.refused); pending = [int]$a.json.pending_jobs; error = $(if ($a.json) { $null } else { Tail $a.tail }) }
                $r = Run @('event', '-Step', 'report', '-Staging', $Out)
                $mid += @{ step = 'report'; ok = [bool]$r.json; jobs = [int]$r.json.jobs; created = [int]$r.json.created; merged = [int]$r.json.merged; linked = [int]$r.json.linked; refused = [int]$r.json.refused }
            }
            'findings' {
                $a = Run @('finding', '-Step', 'apply', '-Staging', $Out, '-Apply')
                $aLock = Batch-LockRefusal $a
                if ($aLock) {
                    $gate = Gate
                    Fail-PlanningStop 'apply' $aLock @{ mid = $mid; gate = $gate; refusal = $aLock.refusal }
                    break
                }
                $mid += @{ step = 'apply'; ok = [bool]$a.json; applied = [int]$a.json.applied_jobs; created = [int]$a.json.created; linked = [int]$a.json.linked; refused = (Count-Of $a.json.refused); pending = [int]$a.json.pending_jobs; error = $(if ($a.json) { $null } else { Tail $a.tail }) }
                $r = Run @('finding', '-Step', 'report', '-Staging', $Out)
                $mid += @{ step = 'report'; ok = [bool]$r.json; jobs = [int]$r.json.jobs; created = [int]$r.json.created; linked = [int]$r.json.linked; refused = [int]$r.json.refused }
            }
            'explain' {
                $argv = @('model', '-Step', 'explain-report', '-Work', (Join-Path $Out 'explain'), '-Staging', $Out) + (Limits-Args)
                $r = Run $argv
                $mid += @{ step = 'report'; ok = [bool]$r.json; models = (Count-Of $r.json.models) }
            }
        }
        # The layer's own apply/report counts, flat (#180): the module logs them, never a nested mid line. A zero
        # is left out: the module reads an absent count as zero, and the line stays short.
        $midLine = @{}
        $applied = @($mid | Where-Object { $_.step -eq 'apply' }) | Select-Object -First 1
        if ($applied) {
            if ([int]$applied.created) { $midLine.mid_applied_created = [int]$applied.created }
            if ([int]$applied.merged) { $midLine.mid_applied_merged = [int]$applied.merged }
            if ([int]$applied.linked) { $midLine.mid_applied_linked = [int]$applied.linked }
            if ([int]$applied.refused) { $midLine.mid_applied_refused = [int]$applied.refused }
        }
        $reported = @($mid | Where-Object { $_.step -eq 'report' }) | Select-Object -First 1
        if ($reported) {
            if ([int]$reported.models) { $midLine.mid_report_models = [int]$reported.models }
            if ([int]$reported.jobs) { $midLine.mid_report_jobs = [int]$reported.jobs }
        }
        if ($Layer -eq 'terms') {
            $u = $units[0]
            $a = Run @('terms', '-Step', 'apply', '-Staging', $Out, '-Token', "$($u.token)", '-Apply')
            $aLock = Batch-LockRefusal $a
            $gate = Gate
            if ($aLock) {
                Fail-PlanningStop 'apply' $aLock @{ mid = $mid; gate = $gate; refusal = $aLock.refusal }
                break
            }
            if ($a.json -and ($a.json.ok -eq $true -or $a.json.state -eq 'done')) {
                $results += @{ unit = $u.id; status = 'done'; terms = [int]$a.json.terms; card = "$($a.json.card)" }
            } else {
                $cause = if ($a.json -and $a.json.error) { "$($a.json.error)" } else { Tail $a.tail }
                $status = Fail-Unit $u $cause 'check_failed'
                $results += @{ unit = $u.id; status = $status; cause = (Tail $cause) }
            }
            Reply (@{ ok = $true; layer = 'terms' } + (Results-Line $results) + @{ terms = [int]$a.json.terms; with_avoid = [int]$a.json.with_avoid; card = "$($a.json.card)" } + (Gate-Fields $gate)) @{ results = $results; mid = $mid; report = $a.json; gate = $gate }
        }
        # finish-batch takes the whole layer's held units in one process (-UnitsFile); check typed
        # planning-stop refusal BEFORE any aggregation, continuation or recovery.
        $pending = @($units)
        $all = @(); $tails = @(); $readable = $true; $lockRefusal = $null; $fatalStop = $null; $round = 0
        while ($pending.Count) {
            $round++
            $uFile = if ($round -eq 1) { Join-Path $Work 'units.json' } else { $n = "units-rem-$round.json"; Save $n $pending; Join-Path $Work $n }
            $one = Run @('finish-batch', '-Out', $Out, '-UnitsFile', $uFile)
            $tails += $one.tail
            $lockRefusal = Batch-LockRefusal $one
            if ($one.json -and $one.json.kind -eq 'finish_batch') {
                $all += @($one.json.results | Where-Object { $_ -and $_.status -ne 'not_run' })
            }
            if ($lockRefusal) { break }
            if (-not $one.json -or $one.json.kind -ne 'finish_batch') { $readable = $false; break }
            $stop = "$($one.json.stop_reason)"
            $notRun = [int]$one.json.not_run
            if ($stop -eq 'transport' -or $stop -eq 'archive_identity') {
                $fatalStop = "${stop}: $notRun $Layer unit(s) not run"
                break
            }
            if ($stop -eq 'budget' -or $notRun -gt 0) {
                $nextPending = Read-Remaining $one.json
                if ($null -eq $nextPending -or $nextPending.Count -eq 0) {
                    $fatalStop = "budget_continuation: missing remaining detail for $notRun $Layer unit(s)"
                    break
                }
                if ($nextPending.Count -ge $pending.Count) {
                    $fatalStop = "budget_continuation: remaining set did not shrink ($($nextPending.Count) >= $($pending.Count))"
                    break
                }
                $pending = $nextPending
                continue
            }
            break
        }
        if ($lockRefusal) {
            foreach ($r in $all) {
                $entry = @{ unit = "$($r.unit)"; token = $r.token; status = "$($r.status)" }
                foreach ($k in @('disposition', 'code', 'cause', 'from')) { if ($r.$k) { $entry[$k] = "$($r.$k)" } }
                $results += $entry
            }
            $gate = Gate
            Fail-PlanningStop 'finish' $lockRefusal @{ results = $results; mid = $mid; gate = $gate; refusal = $lockRefusal.refusal }
            break
        }
        if ($fatalStop) {
            foreach ($r in $all) {
                $entry = @{ unit = "$($r.unit)"; token = $r.token; status = "$($r.status)" }
                foreach ($k in @('disposition', 'code', 'cause', 'from')) { if ($r.$k) { $entry[$k] = "$($r.$k)" } }
                $results += $entry
            }
            $gate = Gate
            Fail 'finish' $fatalStop @{ results = $results; mid = $mid; gate = $gate } $midLine
            break
        }
        $batch = @{ tail = ($tails -join "`n"); json = $(if ($readable) { [pscustomobject]@{ kind = 'finish_batch'; results = $all } } else { $null }) }
        if (-not $batch.json -or $batch.json.kind -ne 'finish_batch') {
            # The batch line came back unreadable: the ledger decides each unit before any fault.
            $states = @(Unit-States ((@($units | ForEach-Object { $_.id })) -join ','))
            if ($states.Count) {
                $read = @()
                foreach ($st in $states) {
                    $u = @($units | Where-Object { $_.id -eq $st.id }) | Select-Object -First 1
                    if ($st.state -eq 'done' -or $st.state -eq 'noop') { $read += @{ unit = $st.id; token = $st.token; status = 'done'; from = 'ledger' }; continue }
                    $cause = "missing_result: the completion line was not readable: $(Tail $batch.tail)"
                    $status = Fail-Unit $u $cause
                    $read += @{ unit = $st.id; token = $st.token; status = $status; cause = (Tail $cause) }
                }
                $gate = Gate
                Reply ((@{ ok = $true; layer = $Layer } + (Results-Line $read) + $midLine + (Gate-Fields $gate))) @{ results = $read; mid = $mid; gate = $gate }
            }
            Fail 'finish-batch' (Tail $batch.tail) @{ mid = $mid; units = $units } $midLine
        }
        foreach ($r in @($batch.json.results)) {
            if (-not $r) { continue }
            $entry = @{ unit = "$($r.unit)"; token = $r.token; status = "$($r.status)" }
            foreach ($k in @('disposition', 'code', 'cause', 'from')) { if ($r.$k) { $entry[$k] = "$($r.$k)" } }
            $results += $entry
        }
        $gate = Gate
        Reply (@{ ok = $true; layer = $Layer } + (Results-Line $results) + $midLine + (Gate-Fields $gate)) @{ results = $results; mid = $mid; gate = $gate }
    }
}





