<#
.SYNOPSIS
  The digest layers' deterministic command chains, one step per call (#175). A DeepSeek drive job runs
  one step and replies with its last line, so no Claude agent relays a single command.

  pwsh -NoProfile -File layer-steps.ps1 -Step start|apply|finish -Out <staging> -Layer <layer> -Owner <label> [options]

  start   claim a bounded block of the layer's ready units, run the layer's own start or plan command for
          the block, and list the DeepSeek jobs it printed
                                                          -> layer-steps\<layer>\units.json, start.json, jobs.json
  apply   the second content round of the explain layer: apply the drafts and list the fix jobs it prints
                                                          -> layer-steps\<layer>\apply-jobs.json
  finish  record every held unit's checked completion, fault the units that cannot finish, then run the
          layer's own apply and report commands and its ledger gate

  The Cards layer's work is the checked finish alone: its `start` claims and finishes a block in one
  command and has no finish step. Every step prints one compact JSON line.

  A unit whose completion the owner's Lock refused keeps that refusal as its content fault and stops the
  step: it prints {ok:false, code:"card_locked", ...} and the module halts the stage with it.
#>
param(
    [Parameter(Mandatory)][ValidateSet('start', 'apply', 'finish')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [Parameter(Mandatory)][ValidateSet('digest', 'link', 'cards', 'events', 'findings', 'explain', 'terms')][string]$Layer,
    [string]$Owner = '',
    [ValidateRange(1, 64)][int]$Take = 8,
    [ValidateRange(0, 600)][int]$Wait = 0,
    [ValidateRange(0, 7200)][int]$Lease = 0,
    [ValidateRange(0, 1000)][int]$MaxParts = 0,
    [string]$Limits = '',
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
$Out = $Out.TrimEnd('\', '/')
$Work = Join-Path (Join-Path $Out 'layer-steps') $Layer
New-Item -ItemType Directory -Force $Work | Out-Null
if (-not $Owner) { $Owner = "$Layer steps" }

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
    $text = if ($list.Count) { $list | ConvertTo-Json -Depth 64 } else { '[]' }
    Set-Content -LiteralPath (Join-Path $Work $Name) -Value $text -Encoding utf8
}
function Load($Name) { $p = Join-Path $Work $Name; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64 } }
function Reply($Value) { $Value | ConvertTo-Json -Depth 16 -Compress; exit 0 }
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
# The owner's typed refusal of an answer (the archive's `card_locked` JSON).
function Lock-Refusal([string]$Text) {
    foreach ($line in "$Text".Split("`n")) {
        $t = $line.Trim()
        if (-not $t.StartsWith('{')) { continue }
        try { $v = $t | ConvertFrom-Json -Depth 16 } catch { continue }
        if ($v.code -eq 'card_locked' -or $v.error -eq 'card_locked') { return $v }
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
# The layer's own per-block start or plan command, and the jobs it lists.
function Start-Block($Units) {
    $firstUnit = $Units[0]
    if ($Layer -eq 'digest' -or $Layer -eq 'link') {
        $startName = if ($Layer -eq 'digest') { 'group-start' } else { 'link-start' }
        return @($Units | ForEach-Object {
                $n = Group-Of $_.id
                $s = Run @($startName, '-Out', $Out, '-Group', "$n", '-Token', "$($_.token)")
                [pscustomobject]@{ unit = $_; group = $n; line = $s; jobs = (Jobs-Of $s.json); error = $(if ($s.json) { $null } else { (Tail $s.tail) }) }
            })
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
# The checked completion command of one held unit (the digest and link layers).
function Finish-Unit($Unit, [string]$Draft) {
    $finishName = if ($Layer -eq 'digest') { 'group-finish' } else { 'link-finish' }
    Run @($finishName, '-Out', $Out, '-Group', "$(Group-Of $Unit.id)", '-Token', "$($Unit.token)", '-Draft', $Draft)
}

$units = @()
$results = @()

switch ($Step) {
    'start' {
        if ($Layer -eq 'cards') {
            # The Cards layer's work is the checked finish alone: one command claims the ready checks and
            # records each one's completion.
            $argv = @('finish-batch', '-Out', $Out, '-Layer', 'cards', '-Take', "$Take", '-Owner', $Owner)
            if ($Wait -gt 0) { $argv += @('-Wait', "$Wait") }
            if ($Lease -gt 0) { $argv += @('-Lease', "$Lease") }
            $line = Run $argv
            if (-not $line.json -or $line.json.kind -ne 'finish_batch') {
                # The line came back unreadable: the ledger decides. Finish whatever this owner still holds;
                # the gate below says whether anything is still open.
                $held = @(Held-Units)
                Save 'units.json' @()
                if ($held.Count) {
                    $list = (@($held | ForEach-Object { "$($_.id):$($_.token)" }) -join ',')
                    $again = Run @('finish-batch', '-Out', $Out, '-Units', $list)
                    Reply @{ ok = $true; layer = 'cards'; claimed = $held.Count; units = $held.Count; jobs = 0; done = @(); failed = @(); results = @($again.json.results); gate = (Gate) }
                }
                Reply @{ ok = $true; layer = 'cards'; claimed = 0; units = 0; jobs = 0; done = @(); failed = @(); results = @(); gate = (Gate) }
            }
            $done = @(); $failed = @()
            foreach ($r in @($line.json.results)) {
                if (-not $r) { continue }
                if ($r.status -eq 'done' -or $r.status -eq 'partial') { $done += $r.unit } else { $failed += $r.unit }
            }
            Save 'units.json' @()
            Reply @{ ok = $true; layer = 'cards'; claimed = [int]$line.json.claimed; units = [int]$line.json.claimed; jobs = 0; done = $done; failed = $failed; results = @($line.json.results); gate = (Gate) }
        }
        $claimArgs = @('ledger', 'next', '-Out', $Out, '-Layer', $Layer, '-Take', "$Take", '-Brief', '-Owner', $Owner)
        if ($Wait -gt 0) { $claimArgs += @('-Wait', "$Wait") }
        if ($Lease -gt 0) { $claimArgs += @('-Lease', "$Lease") }
        # A claim that fails is tried three times before the layer hears of it (a refused claim while parallel
        # stages change the archive is transient).
        $claim = $null
        for ($try = 1; $try -le 3; $try++) {
            $claim = Run $claimArgs
            if ($claim.json -or $claim.code -eq 0) { break }
        }
        $units = @()
        $readable = $false
        if ($claim.json -and $claim.json.units -is [array]) {
            $units = @($claim.json.units | Where-Object { $_ -and $_.id })
            $readable = $true
        }
        if (-not $readable) {
            # A claim line the tool cut is never an empty claim: the units this owner still holds come from
            # the ledger itself.
            $units = @(Held-Units)
            if (-not $units.Count -and -not $claim.json) { Reply @{ ok = $false; step = 'claim'; error = (Tail $claim.tail) } }
        }
        Save 'units.json' $units
        if (-not $units.Count) { Reply @{ ok = $true; layer = $Layer; units = 0; claimed = 0; jobs = 0; gate = (Gate) } }

        $blocks = @(Start-Block $units)
        $bad = @($blocks | Where-Object { $_.error })
        if ($bad.Count) { Reply @{ ok = $false; step = "$Layer start"; error = (Tail $bad[0].error) } }
        $starts = @()
        $allJobs = @()
        foreach ($b in $blocks) {
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
        Save 'jobs.json' $allJobs
        $extra = @{}
        if ($Layer -eq 'events' -or $Layer -eq 'findings') {
            $extra.plan = @{ candidates = [int]$blocks[0].line.json.candidates; groups = [int]$blocks[0].line.json.groups; jobs = @($blocks[0].jobs).Count }
        }
        if ($Layer -eq 'explain') { $extra.plan = @{ parts = [int]$blocks[0].line.json.parts; jobs = @($blocks[0].jobs).Count } }
        $reply = @{ ok = $true; layer = $Layer; units = $units.Count; claimed = $units.Count; jobs = @($allJobs).Count; file = (Join-Path $Work 'jobs.json'); gate = (Gate) }
        Reply ($reply + $extra)
    }
    'apply' {
        if ($Layer -ne 'explain') { Reply @{ ok = $false; step = 'apply'; error = "the $Layer layer has no apply step" } }
        $units = @(Load 'units.json')
        if (-not $units.Count) { Reply @{ ok = $true; layer = $Layer; jobs = 0; applied = 0; file = $null; gate = (Gate) } }
        $argv = @('model', '-Step', 'explain-apply', '-Work', (Join-Path $Out 'explain'), '-Staging', $Out) + (Limits-Args)
        $r = Run $argv
        if (-not $r.json) { Reply @{ ok = $false; step = 'explain-apply'; error = (Tail $r.tail) } }
        $firstUnit = $units[0]
        $jobs = @()
        foreach ($j in @(Jobs-Of $r.json)) {
            $j | Add-Member -NotePropertyName unit -NotePropertyValue $firstUnit.id -Force
            $j | Add-Member -NotePropertyName token -NotePropertyValue $firstUnit.token -Force
            $jobs += $j
        }
        Save 'apply-jobs.json' $jobs
        Reply @{ ok = $true; layer = $Layer; jobs = @($jobs).Count; applied = [int]$r.json.applied; refused = @($r.json.refused).Count; blocked = @($r.json.blocked).Count; file = (Join-Path $Work 'apply-jobs.json'); gate = (Gate) }
    }
    'finish' {
        $units = @(Load 'units.json')
        if ($Layer -eq 'cards' -or -not $units.Count) { Reply @{ ok = $true; layer = $Layer; results = @(); gate = (Gate) } }

        if ($Layer -eq 'digest' -or $Layer -eq 'link') {
            $starts = @(Load 'start.json')
            $done = @(); $failed = @(); $lock = $null
            foreach ($u in $units) {
                $s = @($starts | Where-Object { $_.unit -eq $u.id }) | Select-Object -First 1
                $draft = if ($s) { "$($s.draft)" } else { '' }
                $f = Finish-Unit $u $draft
                if (-not $f.json) {
                    # A cut completion line asks the ledger before any fault: a unit it holds done is done.
                    $said = @(Unit-States $u.id) | Select-Object -First 1
                    if ($said -and ($said.state -eq 'done' -or $said.state -eq 'noop')) {
                        $results += @{ unit = $u.id; token = $u.token; group = (Group-Of $u.id); status = 'done'; from = 'ledger'; jobs = $(if ($s) { [int]$s.expected_jobs } else { 0 }) }
                        $done += $u.id
                        continue
                    }
                    $cause = "missing_result: the completion line was not readable: $(Tail $f.tail)"
                    $status = Fail-Unit $u $cause
                    $results += @{ unit = $u.id; token = $u.token; group = (Group-Of $u.id); status = $status; cause = (Tail $cause) }
                    $failed += $u.id
                    continue
                }
                if (Finished $f.json) {
                    $entry = @{ unit = $u.id; token = $u.token; group = (Group-Of $u.id); status = $(if ($f.json.status -eq 'partial') { 'partial' } else { 'done' }); jobs = $(if ($s) { [int]$s.expected_jobs } else { 0 }) }
                    if ($f.json.disposition) { $entry['disposition'] = "$($f.json.disposition)" }
                    if ($null -ne $f.json.owned) { $entry['owned'] = @($f.json.owned).Count }
                    if ($f.json.recovered -or ($s -and $s.recovered)) { $entry['recovered'] = $true }
                    if ($Layer -eq 'link') {
                        $entry['counts'] = $f.json.counts
                        $entry['quality'] = $f.json.quality
                        $entry['changed'] = @($f.json.changed)
                    }
                    $results += $entry
                    $done += $u.id
                    continue
                }
                $said = Lock-Refusal ("$(if ($f.json) { $f.json | ConvertTo-Json -Depth 16 -Compress })`n$($f.tail)")
                if ($said) {
                    $null = Fail-Unit $u "$($said.message) $($said | ConvertTo-Json -Compress)" 'card_locked'
                    $results += @{ unit = $u.id; token = $u.token; group = (Group-Of $u.id); status = 'failed'; code = 'card_locked'; cause = (Tail "$($said.message)") }
                    $failed += $u.id
                    $lock = @{ unit = $u.id; message = "$($said.message)"; refusal = $said }
                    break
                }
                $cause = if ($f.json -and $f.json.fault) { "$($f.json.fault)" } elseif ($f.json -and $f.json.error) { "$($f.json.error)" } else { Tail $f.tail }
                $status = Fail-Unit $u $cause
                $results += @{ unit = $u.id; token = $u.token; group = (Group-Of $u.id); status = $status; cause = (Tail $cause) }
                $failed += $u.id
            }
            if ($lock) { Reply @{ ok = $false; layer = $Layer; code = 'card_locked'; error = "card_locked: $($lock.message)"; unit = $lock.unit; refusal = $lock.refusal; results = $results; gate = (Gate) } }
            Reply @{ ok = $true; layer = $Layer; results = $results; done = $done; failed = $failed; gate = (Gate) }
        }

        # The layers whose completions finish-batch runs: apply and report first, then every held unit's
        # checked completion in one command.
        $mid = @()
        switch ($Layer) {
            'events' {
                $a = Run @('event', '-Step', 'apply', '-Staging', $Out, '-Apply')
                $mid += @{ step = 'apply'; ok = [bool]$a.json; applied = [int]$a.json.applied_jobs; created = [int]$a.json.created; merged = [int]$a.json.merged; linked = [int]$a.json.linked; refused = @($a.json.refused).Count; pending = [int]$a.json.pending_jobs; error = $(if ($a.json) { $null } else { Tail $a.tail }) }
                $r = Run @('event', '-Step', 'report', '-Staging', $Out)
                $mid += @{ step = 'report'; ok = [bool]$r.json; jobs = [int]$r.json.jobs; created = [int]$r.json.created; merged = [int]$r.json.merged; linked = [int]$r.json.linked; refused = [int]$r.json.refused }
            }
            'findings' {
                $a = Run @('finding', '-Step', 'apply', '-Staging', $Out, '-Apply')
                $mid += @{ step = 'apply'; ok = [bool]$a.json; applied = [int]$a.json.applied_jobs; created = [int]$a.json.created; linked = [int]$a.json.linked; refused = @($a.json.refused).Count; pending = [int]$a.json.pending_jobs; error = $(if ($a.json) { $null } else { Tail $a.tail }) }
                $r = Run @('finding', '-Step', 'report', '-Staging', $Out)
                $mid += @{ step = 'report'; ok = [bool]$r.json; jobs = [int]$r.json.jobs; created = [int]$r.json.created; linked = [int]$r.json.linked; refused = [int]$r.json.refused }
            }
            'explain' {
                $argv = @('model', '-Step', 'explain-report', '-Work', (Join-Path $Out 'explain'), '-Staging', $Out) + (Limits-Args)
                $r = Run $argv
                $mid += @{ step = 'report'; ok = [bool]$r.json; models = @($r.json.models).Count }
            }
        }
        if ($Layer -eq 'terms') {
            $u = $units[0]
            $a = Run @('terms', '-Step', 'apply', '-Staging', $Out, '-Token', "$($u.token)", '-Apply')
            if ($a.json -and ($a.json.ok -eq $true -or $a.json.state -eq 'done')) {
                $results += @{ unit = $u.id; status = 'done'; terms = [int]$a.json.terms; card = "$($a.json.card)" }
            } else {
                $cause = if ($a.json -and $a.json.error) { "$($a.json.error)" } else { Tail $a.tail }
                $status = Fail-Unit $u $cause 'check_failed'
                $results += @{ unit = $u.id; status = $status; cause = (Tail $cause) }
            }
            Reply @{ ok = $true; layer = 'terms'; results = $results; mid = $mid; report = $a.json; gate = (Gate) }
        }
        $list = (@($units | ForEach-Object { "$($_.id):$($_.token)" }) -join ',')
        $batch = Run @('finish-batch', '-Out', $Out, '-Units', $list)
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
                Reply @{ ok = $true; layer = $Layer; results = $read; mid = $mid; gate = (Gate) }
            }
            Reply @{ ok = $false; step = 'finish-batch'; error = (Tail $batch.tail); mid = $mid }
        }
        foreach ($r in @($batch.json.results)) {
            if (-not $r) { continue }
            $entry = @{ unit = "$($r.unit)"; token = $r.token; status = "$($r.status)" }
            foreach ($k in @('disposition', 'code', 'cause', 'from')) { if ($r.$k) { $entry[$k] = "$($r.$k)" } }
            $results += $entry
        }
        $lockRefusal = Lock-Refusal ($batch.json | ConvertTo-Json -Depth 16 -Compress)
        if ($lockRefusal) { Reply @{ ok = $false; layer = $Layer; code = 'card_locked'; error = "card_locked: $($lockRefusal.message)"; refusal = $lockRefusal; results = $results; mid = $mid; gate = (Gate) } }
        Reply @{ ok = $true; layer = $Layer; results = $results; mid = $mid; gate = (Gate) }
    }
}




