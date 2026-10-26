#Requires -Version 7.5
<#
.SYNOPSIS
  The digest layers' deterministic steps (layer-steps.ps1, #175): one script call runs a whole chain and prints
  one compact JSON line.

.DESCRIPTION
  A stand-in zt-digest answers the tool's commands from a small table, so the suite proves the step's own
  behaviour without an archive, a model or a provider:
  - `start` claims a bounded block, runs each held unit's start (digest, link) or the layer's plan
    (events, findings, explain, terms), lists the DeepSeek jobs and saves units, starts and jobs;
  - a claim that fails is tried three times, and a claim line that lost its units is answered from the ledger
    by owner;
  - the Cards layer's `start` claims and finishes a block in one command;
  - `apply` writes the explanation drafts and lists the fix jobs;
  - `finish` records every held unit's checked completion, faults the rest, runs the layer's apply/report and
    the ledger gate, and stops with the owner's typed `card_locked` refusal.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$script = Join-Path (Split-Path $here -Parent) 'layer-steps.ps1'
$failed = 0
$ok = 0
function Assert([bool]$condition, [string]$name) {
    if ($condition) { $script:ok++; Write-Host "ok: $name" } else { $script:failed++; Write-Host "FAIL: $name" }
}
$scratch = Join-Path ([IO.Path]::GetTempPath()) "layer-steps-test-$PID"
if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch }
$tool = Join-Path $scratch 'fake-zt-digest.ps1'
$out = Join-Path $scratch 'out'
New-Item -ItemType Directory -Force $out | Out-Null
$calls = Join-Path $scratch 'calls.txt'
$count = Join-Path $scratch 'claim-count.txt'
$toolBody = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$ErrorActionPreference = 'Stop'
$args2 = $Rest
Add-Content -LiteralPath $env:LS_CALLS -Value ($args2 -join ' ') -Encoding utf8
$cmd = "$($args2[0]) $($args2[1])"
$layer = ''
if (($args2 -join ' ') -match '-Layer (\w+)') { $layer = $Matches[1]; Set-Content -LiteralPath $env:LS_LAYER -Value $layer }
elseif (Test-Path -LiteralPath $env:LS_LAYER) { $layer = (Get-Content -LiteralPath $env:LS_LAYER -Raw).Trim() }
function Units([string]$l) {
    switch ($l) {
        'digest' { return @(@{ id = 'digest/g001@1'; layer = 'digest'; token = 1; input_hash = 'ih-1' }, @{ id = 'digest/g002@1'; layer = 'digest'; token = 2; input_hash = 'ih-2' }) }
        'link' { return @(@{ id = 'link/g001@1'; layer = 'link'; token = 1; input_hash = 'ih-l1' }) }
        'events' { return @(@{ id = 'events/g001@1'; layer = 'events'; token = 1; input_hash = 'ih-e1' }) }
        'findings' { return @(@{ id = 'findings/g001@1'; layer = 'findings'; token = 1; input_hash = 'ih-f1' }) }
        'explain' { return @(@{ id = 'explain/g001-m0000@1'; layer = 'explain'; token = 1; input_hash = 'ih-x1'; inputs = @{ model = 'Key/2|c' } }) }
        'terms' { return @(@{ id = 'terms/book@1'; layer = 'terms'; token = 1; input_hash = 'ih-t1' }) }
    }
    return @()
}
switch -Regex ($cmd) {
    '^ledger next' {
        $n = 0
        if (Test-Path -LiteralPath $env:LS_COUNT) { $n = [int](Get-Content -LiteralPath $env:LS_COUNT -Raw) }
        $n++
        Set-Content -LiteralPath $env:LS_COUNT -Value $n
        if ($env:LS_CLAIM_FAIL -and $n -le [int]$env:LS_CLAIM_FAIL) { Write-Output 'zt-digest ledger: archive changed while checking preparation'; exit 1 }
        if ($env:LS_CLAIM_STRING) { Write-Output (ConvertTo-Json -Compress @{ schema_version = 1; layer = $layer; units = "(2 units claimed)" }); exit 0 }
        if ($env:LS_NO_UNITS) { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; layer = $layer; units = @() }); exit 0 }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; layer = $layer; units = @(Units $layer) })
        exit 0
    }
    '^ledger status' {
        $held = ($args2 -join ' ') -match '-Held'
        $u = @(Units $layer)
        if ($held) { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; units = $u }) }
        else { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; units = @($u | ForEach-Object { @{ id = $_.id; token = $_.token; state = $(if ($env:LS_STATE) { $env:LS_STATE } else { 'done' }); input_hash = $_.input_hash } }) }) }
        exit 0
    }
    '^ledger gate' { Write-Output (ConvertTo-Json -Compress @{ complete = $true; counts = @{ total = 2; done = 2; failed = 0; open = 0 } }); exit 0 }
    '^ledger fail' {
        $flat = $args2 -join ' '
        Add-Content -LiteralPath $env:LS_CALLS -Value "FAULT $flat" -Encoding utf8
        if ($flat -match 'card_locked') { Write-Output (ConvertTo-Json -Compress @{ state = 'failed' }) } else { Write-Output (ConvertTo-Json -Compress @{ state = 'retry' }) }
        exit 0
    }
    '^group-start|^link-start' {
        $job = Join-Path $env:LS_OUT 'job.json'
        Set-Content -LiteralPath $job -Value (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; jobs = @(@{ id = 'j1'; files = @(); prompt = 'p' }); expected_jobs = 1 }) -Encoding utf8
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; job = $job; draft = (Join-Path $env:LS_OUT 'draft.json'); expected_jobs = 1; recovered = $false })
        exit 0
    }
    '^group-finish|^link-finish' {
        if ($env:LS_LOCK) { Write-Output (ConvertTo-Json -Compress @{ ok = $false; code = 'card_locked'; error = 'card_locked'; message = 'the Literature tree Key is locked by the owner' }); exit 2 }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; state = 'done'; complete = $true; disposition = 'verified'; owned = @('Key/1|a') })
        exit 0
    }
    '^finish-batch' {
        $claimed = 0
        if (($args2 -join ' ') -match '-Layer cards') { $claimed = 2 }
        $results = @(@{ unit = 'cards/g001@1'; token = 1; status = 'done'; disposition = 'verified' }, @{ unit = 'cards/g002@1'; token = 2; status = 'done'; disposition = 'verified' })
        if (($args2 -join ' ') -match '-Units') { $results = @(@{ unit = 'events/g001@1'; token = 1; status = 'done'; disposition = 'verified' }) }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; kind = 'finish_batch'; units = $results.Count; done = $results.Count; claimed = $claimed; results = $results })
        exit 0
    }
    '^event |^finding ' {
        if (($args2 -join ' ') -match '-Step plan') {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; step = 'plan'; candidates = 3; groups = 1; jobs = @(@{ id = 'ev-01'; files = @(); prompt = 'p' }) })
        }
        elseif (($args2 -join ' ') -match '-Step report') {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; step = 'report'; jobs = 1; created = 1; merged = 0; linked = 1; refused = 0 })
        }
        else { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; step = 'apply'; applied_jobs = 1; created = 1; merged = 0; linked = 1; refused = @(); pending_jobs = 0 }) }
        exit 0
    }
    '^model ' {
        if (($args2 -join ' ') -match 'explain-apply') { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; applied = 2; refused = @(); blocked = @(); jobs = @(@{ id = 'x+2'; files = @(); prompt = 'p'; continue = 'x1' }) }) }
        elseif (($args2 -join ' ') -match 'explain-report') { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ models = @(@{ model = 'Key/2|c' }) }) }
        else { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; parts = 3; jobs = @(@{ id = 'x1'; files = @(); prompt = 'p' }) }) }
        exit 0
    }
    '^terms ' {
        if (($args2 -join ' ') -match '-Step apply') { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; state = 'done'; terms = 2; with_avoid = 1; card = 'Key/1|a' }) }
        else { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @(@{ id = 'terms'; files = @(); prompt = 'p' }) }) }
        exit 0
    }
}
Write-Output (ConvertTo-Json -Compress @{ ok = $true })
exit 0
'@
Set-Content -LiteralPath $tool -Value $toolBody -Encoding utf8
$env:LS_CALLS = $calls
$env:LS_COUNT = $count
$env:LS_OUT = $out
$env:LS_LAYER = (Join-Path $scratch 'layer.txt')

function Step([string[]]$argv) {
    Remove-Item -LiteralPath $count -ErrorAction SilentlyContinue
    $text = & pwsh -NoProfile -File $script @argv 2>&1
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { "$_".Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = if ($line) { $line | ConvertFrom-Json -Depth 16 } else { $null }
    [pscustomobject]@{ code = $code; json = $json; text = (@($text) -join ' | ') }
}
$common = @('-Out', $out, '-Tool', $tool)

# 1. start: the digest layer claims two units, runs each group-start, lists both jobs.
$start = Step (@('-Step', 'start', '-Layer', 'digest', '-Take', '8', '-Owner', 'digest block 1', '-Lease', '7200') + $common)
Assert ($start.json -and $start.json.ok -eq $true -and $start.json.units -eq 2 -and $start.json.jobs -eq 2) "digest start: two units claimed and two jobs listed ($($start.text))"
$units = Get-Content -Raw (Join-Path $out 'layer-steps\digest\units.json') | ConvertFrom-Json
$jobs = Get-Content -Raw (Join-Path $out 'layer-steps\digest\jobs.json') | ConvertFrom-Json
Assert (@($units).Count -eq 2 -and @($jobs).Count -eq 2 -and @($jobs)[0].unit -eq 'digest/g001@1' -and @($jobs)[0].token -eq 1) 'digest start: units and jobs carry each job''s own unit and token'
$all = Get-Content -LiteralPath $calls -Raw
Assert (($all -split "`n" | Where-Object { $_ -match '^ledger next .*-Layer digest .*-Take 8 .*-Owner digest block 1 -Lease 7200' }).Count -eq 1) 'digest start: one claim command carries the owner and the two-hour lease'
Assert (($all -split "`n" | Where-Object { $_ -match '^group-start .*-Group 1 -Token 1' }).Count -eq 1 -and ($all -split "`n" | Where-Object { $_ -match '^group-start .*-Group 2 -Token 2' }).Count -eq 1) 'digest start: each held group runs group-start with its group and token'

# 2. finish: each held unit's checked completion, then the gate.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$finish = Step (@('-Step', 'finish', '-Layer', 'digest', '-Owner', 'digest block 1') + $common)
Assert ($finish.json -and $finish.json.ok -eq $true -and @($finish.json.results).Count -eq 2 -and @($finish.json.done).Count -eq 2 -and $finish.json.gate.complete -eq $true) "digest finish: both units complete and the gate settles ($($finish.text))"
$fin = Get-Content -LiteralPath $calls -Raw
Assert (($fin -split "`n" | Where-Object { $_ -match '^group-finish .*-Group 1 -Token 1 -Draft ' }).Count -eq 1 -and ($fin -split "`n" | Where-Object { $_ -match '^ledger gate .*-Layer digest -Brief' }).Count -eq 1) 'digest finish: group-finish runs with the draft the start step bound, then the brief gate'

# 3. the Cards layer claims and finishes a block in one command.
$cards = Step (@('-Step', 'start', '-Layer', 'cards', '-Take', '4', '-Owner', 'cards block 1', '-Lease', '7200') + $common)
Assert ($cards.json -and $cards.json.ok -eq $true -and $cards.json.claimed -eq 2 -and @($cards.json.results).Count -eq 2) "cards start: one finish-batch claims and records the block ($($cards.text))"
$cardCalls = Get-Content -LiteralPath $calls -Raw
Assert ($cardCalls -match 'finish-batch .*-Layer cards -Take 4 -Owner cards block 1 -Lease 7200') 'cards start: the claim-and-finish command carries the owner and lease'

# 4. the owner's Lock stops the finish step with the typed refusal and keeps the unit's content fault.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$env:LS_LOCK = '1'
$locked = Step (@('-Step', 'finish', '-Layer', 'digest', '-Owner', 'digest block 1') + $common)
$env:LS_LOCK = ''
Assert ($locked.json -and $locked.json.ok -eq $false -and $locked.json.code -eq 'card_locked' -and $locked.json.unit -eq 'digest/g001@1') "digest finish: the Lock stops the step with the typed refusal ($($locked.text))"
$lockCalls = Get-Content -LiteralPath $calls -Raw
Assert ($lockCalls -match "ledger fail .*-Unit digest/g001@1 .*-Fault .*card_locked.*") 'digest finish: the refused unit keeps card_locked as its content fault'
Assert (($lockCalls -split "`n" | Where-Object { $_ -match '^group-finish .*-Group 2 ' }).Count -eq 0) 'digest finish: a locked unit stops the step before the next unit'

# 5. events: start plans the block and lists its job; finish applies, reports and records the block.
$env:LS_NO_UNITS = ''
$events = Step (@('-Step', 'start', '-Layer', 'events', '-Take', '4', '-Owner', 'events block 1') + $common)
Assert ($events.json -and $events.json.ok -eq $true -and $events.json.units -eq 1 -and $events.json.jobs -eq 1 -and $events.json.plan.candidates -eq 3) "events start: the plan runs and its job is listed ($($events.text))"
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$evFinish = Step (@('-Step', 'finish', '-Layer', 'events', '-Owner', 'events block 1') + $common)
$evCalls = Get-Content -LiteralPath $calls -Raw
Assert ($evFinish.json -and $evFinish.json.ok -eq $true -and @($evFinish.json.results).Count -eq 1) "events finish: the held unit's checked completion is recorded ($($evFinish.text))"
Assert ($evCalls -match 'event -Step apply -Staging .* -Apply' -and $evCalls -match 'event -Step report' -and $evCalls -match 'finish-batch .*-Units events/g001@1:1') 'events finish: apply, report and the checked completion run in that one step'

# 6. explain: apply lists the fix jobs, and finish records the block.
$env:LS_NO_UNITS = ''
$ex = Step (@('-Step', 'start', '-Layer', 'explain', '-Take', '4', '-Owner', 'explain block 1', '-Limits', '-MaxSentences 12 -MaxChars 400', '-MaxParts', '5') + $common)
Assert ($ex.json -and $ex.json.ok -eq $true -and $ex.json.units -eq 1 -and $ex.json.jobs -eq 1) "explain start: the plan lists one explanation job ($($ex.text))"
$exCalls = Get-Content -LiteralPath $calls -Raw
Assert ($exCalls -match 'model -Step explain .*-MaxSentences 12 -MaxChars 400.*-Models Key/2\|c.*-MaxParts 5') 'explain start: the compile limits, the claimed models and the part cap reach the plan'
$apply = Step (@('-Step', 'apply', '-Layer', 'explain', '-Owner', 'explain block 1', '-Limits', '-MaxSentences 12') + $common)
Assert ($apply.json -and $apply.json.ok -eq $true -and $apply.json.jobs -eq 1 -and $apply.json.applied -eq 2) "explain apply: the drafts are applied and the fix job is listed ($($apply.text))"
$fix = Get-Content -Raw (Join-Path $out 'layer-steps\explain\apply-jobs.json') | ConvertFrom-Json
Assert (@($fix).Count -eq 1 -and (Get-Content -Raw (Join-Path $out 'layer-steps\explain\apply-jobs.json')) -match 'continue.*x1' -and @($fix)[0].unit -eq 'explain/g001-m0000@1') 'explain apply: the fix job keeps its continue and the block''s unit'

# 7. terms: start plans the table, finish applies it and completes the unit.
$terms = Step (@('-Step', 'start', '-Layer', 'terms', '-Take', '1', '-Owner', 'terms block 1') + $common)
$termsFinish = Step (@('-Step', 'finish', '-Layer', 'terms', '-Owner', 'terms block 1') + $common)
Assert ($terms.json -and $terms.json.ok -eq $true -and $termsFinish.json -and $termsFinish.json.ok -eq $true -and @($termsFinish.json.results)[0].status -eq 'done') "terms: the plan and the apply complete the one unit ($($termsFinish.text))"
$tCalls = Get-Content -LiteralPath $calls -Raw
Assert ($tCalls -match 'terms -Step plan -Staging .*-Token 1' -and $tCalls -match 'terms -Step apply -Staging .*-Token 1 -Apply') 'terms: the apply carries the claimed unit''s token'

# 8. a claim that fails is tried three times, and a claim line without its units asks the ledger.
$env:LS_CLAIM_FAIL = '2'
$retry = Step (@('-Step', 'start', '-Layer', 'digest', '-Take', '8', '-Owner', 'digest block 9') + $common)
$env:LS_CLAIM_FAIL = ''
Assert ($retry.json -and $retry.json.ok -eq $true -and $retry.json.units -eq 2) "claim retry: two refused claims are tried again and the block is still claimed ($($retry.text))"
$env:LS_CLAIM_STRING = '1'
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$lost = Step (@('-Step', 'start', '-Layer', 'explain', '-Take', '4', '-Owner', 'explain block 9') + $common)
$env:LS_CLAIM_STRING = ''
Assert ($lost.json -and $lost.json.ok -eq $true -and $lost.json.units -eq 1) "lost claim line: the step reads its held units from the ledger by owner ($($lost.text))"
$lostCalls = Get-Content -LiteralPath $calls -Raw
Assert ($lostCalls -match 'ledger status -Out .* -Held explain block 9') 'lost claim line: the fallback is the ledger read by owner'

# 9. no ready unit is an empty block, never an error.
$env:LS_NO_UNITS = '1'
$empty = Step (@('-Step', 'start', '-Layer', 'digest', '-Take', '8', '-Owner', 'digest block 10') + $common)
$env:LS_NO_UNITS = ''
Assert ($empty.json -and $empty.json.ok -eq $true -and $empty.json.units -eq 0 -and $empty.json.jobs -eq 0) "empty claim: a block with no ready unit is an empty success ($($empty.text))"

Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
Write-Host ''
Write-Host "layer-steps: $ok passed, $failed failed"
if ($failed) { exit 1 } else { exit 0 }



