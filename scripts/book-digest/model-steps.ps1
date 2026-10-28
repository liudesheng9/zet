#Requires -Version 7.5
<#
.SYNOPSIS
  The model layer's deterministic command chains, one step per call (#177, #180). A DeepSeek job runs one step
  and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File model-steps.ps1 -Step start|brief|fill|derive|review|split|close|report|finish -Out <staging>

  start   release the Model claims a stopped run left, list the Model Cards of the book, claim a block of
          up to -Take ready Model units (lease 7200 s), plan the block's architect batches, and name them
  brief   the brief jobs of one batch's models                                  -> drive\jobs.json
  fill    plan-status, the batch's model-deps references, the defer of the models they release, and the
          fill jobs                                                             -> drive\jobs.json
  derive  the Derivation jobs of the batch                                      -> drive\jobs.json
  review  one review job per model: it continues the model's conversation (the math variant, with
          -IndependentReview, starts the reviewer's own `<slug>:review` conversation)  -> drive\jobs.json
  split   apply the review verdicts (and the composers' -Need requests)         -> drive\jobs.json
  close   the build faults back to the writers once; the last round that sends nothing back also writes
          the batch report (-Report)                                            -> drive\jobs.json
  report  one report per model of the batch
  finish  every named unit's checked completion, in one command, and the layer gate

  #180: each step prints ONE compact JSON line: `ok` first, then the counts, the packed plans and the file paths
  the module reads, at most 400 characters. The full data (the tool's own lines, the per-model report rows) goes
  to <staging>\models\drive-lines\<step>.json, named in the line as `detail`. The architect batching is step
  logic now: `start` reads the listing and answers `batches` as `concept:<addresses>|formal:<addresses>`.
  Files stay under <staging>\models\drive.
#>
param(
    [Parameter(Mandatory)][ValidateSet('start', 'brief', 'fill', 'derive', 'review', 'split', 'close', 'report', 'finish')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [string]$Work = '',
    [string]$Models = '',
    [string]$Units = '',
    [string]$CitationKey = '',
    [string]$Level = '',
    [string]$Need = '',
    [string]$BriefDoc = '',
    [string]$FillDoc = '',
    [string]$ReviewDoc = '',
    [string]$Only = '',
    [string]$SkipForms = '',
    [int]$PerConcept = 2,
    [int]$PerSonnet = 8,
    [int]$Take = 8,
    [string]$Owner = 'models',
    [int]$MaxSentences = 12,
    [int]$MaxChars = 400,
    [int]$MaxSteps = 6,
    [int]$MaxExplainSentences = 16,
    [int]$MaxExplainChars = 600,
    [switch]$Rebuild,
    [switch]$Changed,
    [switch]$IndependentReview,
    [switch]$SkipRenew,
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'step-line.ps1')
$Out = $Out.TrimEnd('\', '/')
$ModelWork = Join-Path $Out 'models'
$Drive = Join-Path $ModelWork 'drive'
New-Item -ItemType Directory -Force $Drive | Out-Null
$LineName = $Step
# The book's Citation key: the step's own option, else the prepared run's manifest.json. A resume names only `out`,
# so this is where its model layer finds the book's identity (#181).
function Book-Key([string]$Value) {
    $v = "$Value".Trim()
    if ($v) { return $v }
    $manifest = Join-Path $Out 'manifest.json'
    if (Test-Path -LiteralPath $manifest) {
        try {
            $doc = Get-Content -LiteralPath $manifest -Raw -Encoding utf8 | ConvertFrom-Json -Depth 8
            if ($doc -and "$($doc.citation_key)".Trim()) { return "$($doc.citation_key)".Trim() }
        } catch { }
    }
    return $v
}
$CitationKey = Book-Key $CitationKey
$Limits = "-MaxSentences $MaxSentences -MaxChars $MaxChars -MaxSteps $MaxSteps -MaxExplainSentences $MaxExplainSentences -MaxExplainChars $MaxExplainChars"
# The math variant's own instruction files (base 5.6.5 / math): every compiler step takes them.
$Docs = @()
if ($BriefDoc) { $Docs += @('-BriefDoc', "$BriefDoc") }
if ($FillDoc) { $Docs += @('-FillDoc', "$FillDoc") }
if ($ReviewDoc) { $Docs += @('-ReviewDoc', "$ReviewDoc") }

# One command: its last JSON line (or $null), its exit code and the tail of its output.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = ((@($text) | Select-Object -Last 3) -join ' | ') }
}
function Save($Name, $Value) { $Value | ConvertTo-Json -Depth 64 | Set-Content -LiteralPath (Join-Path $Drive $Name) -Encoding utf8 }
function Load($Name) { $p = Join-Path $Drive $Name; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64 } }
# The step's answer: its compact line and the detail file beside it (#180).
function Reply($Value, $Detail = $null) { Send-StepLine -Line $Value -Detail $Detail -Name $LineName -Out $ModelWork }
function Fail([string]$Where, $Result) { Send-StepFail -Step $Where -Reason "$(@($Result.tail) | Select-Object -Last 1)" -Name $LineName -Out $ModelWork }
# The model a claimed unit names (the model compiler's own reading of a claim's inputs).
function Address($Unit) {
    $inputs = $Unit.inputs
    if (-not $inputs) { return '' }
    if ($inputs.header) { return "$($inputs.header)" }
    if ($inputs.model) { return "$($inputs.model)" }
    return ''
}
# The claimed units this step holds, as the "unit:token" list the tool takes.
function Held() {
    # Not `$units`: a parameter's type constraint also holds for a local of the same name (case-insensitive).
    $heldUnits = @(Load 'units.json' | Where-Object { $_ -and $_.id -and $_.token })
    if ($Units) {
        $wantedUnits = @($Units.Split(',') | Where-Object { $_ })
        $heldUnits = @($heldUnits | Where-Object { $wantedUnits -contains "$($_.id):$($_.token)" })
    }
    return $heldUnits
}
# The held units of the named models (a batch's models), as the tool's unit list.
function Held-Of([string]$Addresses) {
    $wanted = @("$Addresses".Split(',') | Where-Object { $_ })
    $held = @(Load 'units.json' | Where-Object { $_ -and $_.id -and $_.token })
    return @($held | Where-Object { $wanted -contains (Address $_) })
}
# A step of a batch renews the block's claims first: the compiler's chains outlive a claim's lease.
function Renew() {
    if ($SkipRenew) { return }
    $list = (@(Load 'units.json' | Where-Object { $_ -and $_.id -and $_.token }) | ForEach-Object { "$($_.id):$($_.token)" }) -join ','
    if (-not $list) { return }
    Run @('ledger', 'hold', '-Out', $Out, '-Units', "$list", '-Seconds', '1', '-Every', '1') | Out-Null
}
# The jobs one compiler command printed, saved for the batch's writers.
function Jobs($Name, $Result) {
    $jobs = @($Result.json.jobs | Where-Object { $_ })
    Save $Name $jobs
    return (Join-Path $Drive $Name)
}
function Compiler([string]$StepName, [string[]]$Extra = @()) {
    $argv = @('model', '-Step', $StepName, '-Work', $Work, '-Staging', $Out)
    $argv += $Docs
    $argv += $Extra
    return Run $argv
}
# A model's own status, packed as `address:status` pairs (#180): the module reads a short list, never a row
# per model. A status is `done`, `failed` or `skipped`.
# A model's address without its book key (`Key/2|a` -> `2|a`): the step lines name models this way and the module
# adds the key back, so a batch's line fits its 400 characters (live run 2026-10-26: six full addresses overflowed).
function Short([string]$Address) {
    $at = $Address.IndexOf('/')
    if ($at -lt 0) { return $Address }
    return $Address.Substring($at + 1)
}
function Status-List($Rows) {
    return (@(@($Rows) | Where-Object { $_ -and $_.model } | ForEach-Object { $st = if ($_.status) { "$($_.status)" } else { 'done' }; "$(Short "$($_.model)"):$st" }) -join ';')
}

switch ($Step) {
    'start' {
        # A start begins the branch: a Model claim still held belongs to a stopped run. Release it, or the
        # new block waits for a lease that no writer is using.
        $status = Run @('ledger', 'status', '-Out', $Out, '-Layer', 'models')
        foreach ($u in @($status.json.units | Where-Object { $_.state -eq 'claimed' })) {
            Run @('ledger', 'reclaim', '-Out', $Out, '-Unit', "$($u.id)", '-Release') | Out-Null
        }
        $listed = Run @('list-models', '-Staging', $Out, '-All', '-CitationKey', $CitationKey)
        if (-not $listed.json -or -not ($listed.json.models -is [Array])) { Fail 'list' $listed; break }
        $all = @(@($listed.json.models) | Where-Object { $_ -and $_.address })
        Save 'models-all.json' $all
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'models', '-Take', "$Take", '-Owner', $Owner, '-Lease', '7200', '-Brief')
        if (-not $claim.json) { Fail 'claim' $claim; break }
        $claimUnits = @($claim.json.units | Where-Object { $_ -and $_.id -like 'models/*' })
        # A claim line that lost its units (or their Model address) is never read as an empty claim: the
        # units this owner still holds come from the ledger.
        if (-not ($claim.json.units -is [Array]) -or @($claimUnits | Where-Object { -not (Address $_) }).Count) {
            $read = Run @('ledger', 'status', '-Out', $Out, '-Held', $Owner)
            if ($read.json -and ($read.json.units -is [Array])) { $claimUnits = @($read.json.units | Where-Object { $_ -and $_.id -like 'models/*' }) }
        }
        Save 'units.json' $claimUnits
        $gate = Run @('ledger', 'gate', '-Out', $Out, '-Layer', 'models', '-Brief')
        $wanted = @($claimUnits | ForEach-Object { Address $_ } | Where-Object { $_ })
        # The block's models, as the listing holds them: which of them are built and how they batch is this
        # step's plan (a skipped form, an already-built model, a level change), never a second listing.
        $blockModels = @($all | Where-Object { $wanted -contains $_.address })
        Save 'models.json' $blockModels
        # Not `$only`: the `-Only` parameter's type constraint also holds for a local of the same name.
        $onlyList = @("$Only".Split(',') | Where-Object { $_ })
        $skipped = @("$SkipForms".Split(',') | Where-Object { $_ })
        $todo = @($blockModels | Where-Object { $skipped -notcontains "$($_.form)" })
        if ($onlyList.Count) { $todo = @($todo | Where-Object { $onlyList -contains "$($_.address)" }) }
        $todo = @($todo | Where-Object { $Rebuild -or ($Level -and "$($_.level)" -ne $Level) -or -not ([int]$_.parts -gt 0 -and -not [bool]$_.issues) })
        $groupOf = { param($m) $g = [int]$m.group; if (-not $g) { $g = [int]("$($m.address)".Split('/')[1].Split('|')[0]) }; $g }
        $todo = @($todo | Sort-Object @{ Expression = { & $groupOf $_ } }, @{ Expression = { "$($_.address)" } })
        $concept = @($todo | Where-Object { $(if ($Level) { $Level } else { "$($_.level)" }) -eq 'concept' })
        $formal = @($todo | Where-Object { $(if ($Level) { $Level } else { "$($_.level)" }) -ne 'concept' })
        $batches = @()
        for ($i = 0; $i -lt $concept.Count; $i += [Math]::Max(1, $PerConcept)) { $batches += @{ kind = 'concept'; models = @($concept[$i..([Math]::Min($i + [Math]::Max(1, $PerConcept) - 1, $concept.Count - 1))] | ForEach-Object { "$($_.address)" }) } }
        for ($i = 0; $i -lt $formal.Count; $i += [Math]::Max(1, $PerSonnet)) { $batches += @{ kind = 'formal'; models = @($formal[$i..([Math]::Min($i + [Math]::Max(1, $PerSonnet) - 1, $formal.Count - 1))] | ForEach-Object { "$($_.address)" }) } }
        Save 'batches.json' $batches
        # The plan as one short string: `concept:a,b|formal:c` (#180).
        $plan = (@($batches | ForEach-Object { "$($_.kind):$((@($_.models | ForEach-Object { Short "$_" })) -join ',')" }) -join ';')
        $line = @{ ok = $true; units = $claimUnits.Count; claimed = $claimUnits.Count; models = $blockModels.Count; batches = $batches.Count; plan = $plan }
        if ($gate.json -and $gate.json.counts) { $line.gate_complete = ($gate.json.complete -eq $true); $line.gate_open = [int]$gate.json.counts.open }
        Reply $line @{ units = $claimUnits; models = $blockModels; batches = $batches; gate = $gate.json }
    }
    'brief' {
        Renew
        $extra = @('-Models', $Models)
        if ($Rebuild) { $extra += '-Rebuild' }
        if ($Level) { $extra += @('-Level', $Level) }
        $r = Compiler 'brief' ($extra + $Limits.Split(' '))
        if (-not $r.json) { Fail 'brief' $r; break }
        $file = Jobs 'jobs.json' $r
        Reply @{ ok = $true; step = 'brief'; jobs = @($r.json.jobs).Count; file = $file; skipped = (@($r.json.skipped | Where-Object { $_ } | ForEach-Object { Short "$(if ($_.model) { $_.model } else { $_ })" }) -join ',') } $r.json
    }
    'fill' {
        Renew
        $status = Compiler 'plan-status'
        if (-not $status.json -or -not ($status.json.plans -is [Array])) { Fail 'plan-status' $status; break }
        $plans = @($status.json.plans)
        Save 'plan-status.json' $plans
        # R233/R234: the batch's planned models register their references before the fill: one command for
        # the block, then each unit the command does not answer on its own, as the ledger decides it.
        $held = if ($Models) { @(Held-Of $Models) } else { @(Held) }
        $deferred = @()
        if ($held.Count) {
            $planned = @($held | Where-Object { $at = Address $_; $plans | Where-Object { $_.model -eq $at -and $_.status -eq 'planned' } })
            if ($planned.Count) {
                $batch = Run @('model-deps', '-Out', $Out, '-Units', (($planned | ForEach-Object { "$($_.id):$($_.token)" }) -join ','))
                $rows = @()
                if ($batch.json -and $batch.json.kind -eq 'model_deps_batch' -and ($batch.json.results -is [Array])) { $rows = @($batch.json.results) }
                else {
                    # A batch line that did not come back whole is never read as "resolved": the ledger says
                    # which units model-deps already released, and only a unit still claimed runs it alone.
                    Write-Host 'model-deps: no readable batch line; the ledger decides each model.'
                }
                if (-not $rows.Count) {
                    $states = @()
                    $read = Run @('ledger', 'status', '-Out', $Out, '-Units', (($planned | ForEach-Object { "$($_.id)" }) -join ','))
                    if ($read.json -and ($read.json.units -is [Array])) { $states = @($read.json.units) }
                    foreach ($u in $planned) {
                        $at = Address $u
                        $heldRow = @($states | Where-Object { $_ -and $_.id -eq $u.id }) | Select-Object -First 1
                        if ($heldRow -and $heldRow.state -eq 'registered') { $deferred += [pscustomobject]@{ model = $at; why = 'waiting' }; continue }
                        if ($heldRow -and ($heldRow.state -ne 'claimed' -or [int]$heldRow.token -ne [int]$u.token)) { $deferred += [pscustomobject]@{ model = $at; why = 'refused' }; continue }
                        $one = Run @('model-deps', '-Out', $Out, '-Model', "$at", '-Token', "$($u.token)")
                        $rows += [pscustomobject]@{ unit = $u.id; ok = $(if ($one.json) { $one.json.ok } else { $null }); waiting = $(if ($one.json) { $one.json.waiting } else { $null }) }
                    }
                }
                foreach ($u in $planned) {
                    $at = Address $u
                    $row = @($rows | Where-Object { $_ -and $_.unit -eq $u.id }) | Select-Object -First 1
                    if (-not $row) { continue }
                    if ($row.ok -eq $true -and $row.waiting -eq $true) { if (-not ($deferred | Where-Object { $_.model -eq $at })) { $deferred += [pscustomobject]@{ model = $at; why = 'waiting' } } }
                    elseif ($row.ok -eq $false) { if (-not ($deferred | Where-Object { $_.model -eq $at })) { $deferred += [pscustomobject]@{ model = $at; why = 'refused' } } }
                }
            }
        }
        $still = @($plans | Where-Object { $_.status -eq 'planned' })
        $go = @($still | Where-Object { $deferred.model -notcontains $_.model })
        $derive = @($go | Where-Object { [int]$_.derivations -gt 0 }).Count -gt 0
        if ($deferred.Count) {
            $d = Compiler 'defer' @('-Models', (($deferred | ForEach-Object { $_.model } | Select-Object -Unique) -join ','))
            if (-not $d.json -or $d.json.ok -ne $true) { Fail 'defer' $d; break }
        }
        $line = @{
            ok = $true; step = 'fill'; plans = $plans.Count; derive = [bool]$derive
            deferred = (@($deferred | ForEach-Object { "$(Short "$($_.model)"):$($_.why)" }) -join ';')
            statuses = (Status-List $plans)
        }
        if (-not $go.Count) {
            Save 'jobs.json' @()
            $line.jobs = 0
            $line.file = (Join-Path $Drive 'jobs.json')
            Reply $line @{ plans = $plans; deferred = $deferred }
            break
        }
        $a = Compiler 'apply' $Limits.Split(' ')
        if (-not $a.json) { Fail 'apply' $a; break }
        $file = Jobs 'jobs.json' $a
        $line.jobs = @($a.json.jobs).Count
        $line.file = $file
        Reply $line @{ plans = $plans; deferred = $deferred; apply = $a.json }
    }
    'derive' {
        Renew
        $r = Compiler 'derive'
        if (-not $r.json) { Fail 'derive' $r; break }
        $file = Jobs 'jobs.json' $r
        Reply @{ ok = $true; step = 'derive'; jobs = @($r.json.jobs).Count; file = $file } $r.json
    }
    'review' {
        Renew
        $extra = @()
        if ($Changed) { $extra += '-Changed' }
        # #194: the math variant reviews in its own conversation (`<slug>:review`); the economics
        # variant's reviewer continues the model's conversation.
        if ($IndependentReview) { $extra += '-IndependentReview' }
        $r = Compiler 'review' $extra
        if (-not $r.json) { Fail 'review' $r; break }
        $file = Jobs 'jobs.json' $r
        Reply @{ ok = $true; step = 'review'; jobs = @($r.json.jobs).Count; file = $file } $r.json
    }
    'split' {
        Renew
        $extra = @()
        if ($Need) { $extra += @('-Need', $Need) }
        $r = Compiler 'split' $extra
        if (-not $r.json) { Fail 'split' $r; break }
        $file = Jobs 'jobs.json' $r
        Reply @{ ok = $true; step = 'split'; jobs = @($r.json.jobs).Count; file = $file } $r.json
    }
    'close' {
        Renew
        $r = Compiler 'close' @('-Report')
        if (-not $r.json) { Fail 'close' $r; break }
        $file = Jobs 'jobs.json' $r
        $report = $r.json.report
        # Not `$models`: the `-Models` parameter's type constraint also holds for a local of the same name.
        $reportRows = @($report.models | Where-Object { $_ })
        Reply @{
            ok = $true; step = 'close'; jobs = @($r.json.jobs).Count; faults = [int]$r.json.faults
            unowned = @($r.json.unowned | Where-Object { $_ }).Count; report_models = $reportRows.Count
            statuses = (Status-List $reportRows); file = $file
        } @{ report = $report; close = $r.json }
    }
    'report' {
        Renew
        $r = Compiler 'report'
        if (-not $r.json) { Fail 'report' $r; break }
        Save 'report.json' $r.json
        $reportRows = @($r.json.models | Where-Object { $_ })
        Reply @{ ok = $true; step = 'report'; models = $reportRows.Count; statuses = (Status-List $reportRows); file = (Join-Path $Drive 'report.json') } $r.json
    }
    'finish' {
        $held = @(Load 'units.json' | Where-Object { $_ -and $_.id -and $_.token })
        if ($Units) { $held = @(Held) }
        $gate = Run @('ledger', 'gate', '-Out', $Out, '-Layer', 'models', '-Brief')
        $gateFields = @{}
        if ($gate.json -and $gate.json.counts) { $gateFields.gate_complete = ($gate.json.complete -eq $true); $gateFields.gate_open = [int]$gate.json.counts.open }
        if (-not $held.Count) { Reply (@{ ok = $true } + $gateFields) @{ results = @(); gate = $gate.json }; break }
        $list = ($held | ForEach-Object { "$($_.id):$($_.token)" }) -join ','
        $b = Run @('finish-batch', '-Out', $Out, '-Units', "$list")
        if (-not $b.json -or $b.json.kind -ne 'finish_batch' -or -not ($b.json.results -is [Array])) { Fail 'finish-batch' $b; break }
        $results = @($b.json.results)
        $done = @($results | Where-Object { $_.status -eq 'done' -or $_.status -eq 'partial' }).Count
        $failed = @($results | Where-Object { $_.status -ne 'done' -and $_.status -ne 'partial' }).Count
        $partial = @($results | Where-Object { $_.status -eq 'partial' }).Count
        Reply (@{ ok = $true; done = $done; failed = $failed; partial = $partial } + $gateFields) @{ results = $results; gate = $gate.json }
    }
}









