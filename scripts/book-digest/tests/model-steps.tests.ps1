#Requires -Version 7.5
<#
.SYNOPSIS
  The model layer's deterministic steps (model-steps.ps1, #177): one script call runs a whole chain and prints
  one compact JSON line.

.DESCRIPTION
  A stand-in zt-digest answers the tool's commands from a small table, so the suite proves the step's own
  behaviour without an archive, a model or a provider:
  - `start` releases the Model claims a stopped run left, lists the Model Cards, claims a block of up to
    `-Take` ready Model units with the two-hour lease, and names only the models the block builds (skipped
    forms, `-Only`, and already-built models left out);
  - `brief`, `derive`, `review`, `split` and `close` each run their one compiler command and save its jobs;
  - `fill` runs plan-status, the batch's one model-deps command, the defer of the models it releases, and the
    fill, in one process;
  - `finish` records the named units' checked completions in one command and reads the layer gate;
  - a failed tool command is one `ok:false` line, never a half-written step.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$script = Join-Path (Split-Path $here -Parent) 'model-steps.ps1'
$failed = 0
$ok = 0
function Assert([bool]$condition, [string]$name) {
    if ($condition) { $script:ok++; Write-Host "ok: $name" } else { $script:failed++; Write-Host "FAIL: $name" }
}
$scratch = Join-Path ([IO.Path]::GetTempPath()) "model-steps-test-$PID"
if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch }
$tool = Join-Path $scratch 'fake-zt-digest.ps1'
$out = Join-Path $scratch 'out'
$work = Join-Path $out 'models\m-Key_2_c'
New-Item -ItemType Directory -Force $out | Out-Null
$calls = Join-Path $scratch 'calls.txt'
$toolBody = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$ErrorActionPreference = 'Stop'
$flat = $Rest -join ' '
Add-Content -LiteralPath $env:MS_CALLS -Value $flat -Encoding utf8
$cmd = "$($Rest[0]) $($Rest[1])"
$stale = $env:MS_STALE -eq '1'
function Emit($Value) { $Value | ConvertTo-Json -Compress -Depth 16; exit 0 }
switch -Regex ($cmd) {
    '^ledger status' {
        if ($flat -match '-Held') {
            Emit @{ schema_version = 1; owner = 'models block 1'; units = @(
                    @{ id = 'models/g001-m0000@1'; token = 1; input_hash = 'ih-1'; inputs = @{ header = 'Key/2|c' } }
                ) }
        }
        if ($flat -match ' -Units ') {
            Emit @{ schema_version = 1; units = @(@{ id = 'models/g001-m0000@1'; state = 'claimed'; token = 1; input_hash = 'ih-1' }) }
        }
        if ($flat -match '-Layer models') {
            if ($stale) { Emit @{ schema_version = 1; layer = 'models'; units = @(@{ id = 'models/g001-m0000@1'; state = 'claimed'; token = 4 }) } }
            Emit @{ schema_version = 1; layer = 'models'; units = @() }
        }
        Emit @{ schema_version = 1; units = @() }
    }
    '^ledger reclaim' { Emit @{ schema_version = 1; unit = 'models/g001-m0000@1'; state = 'ready'; released = $true } }
    '^ledger next' {
        if ($env:MS_NO_UNITS -eq '1') { Emit @{ schema_version = 1; layer = 'models'; units = @() } }
        if ($env:MS_RENAMED -eq '1') { Emit @{ schema_version = 1; layer = 'models'; units = '(3 units claimed)' } }
        Emit @{ schema_version = 1; layer = 'models'; units = @(
                @{ id = 'models/g001-m0000@1'; layer = 'models'; token = 1; input_hash = 'ih-1'; inputs = @{ header = 'Key/2|c' } },
                @{ id = 'models/g002-m0001@1'; layer = 'models'; token = 1; input_hash = 'ih-2'; inputs = @{ model = 'Key/3|c' } },
                @{ id = 'models/g003-m0002@1'; layer = 'models'; token = 1; input_hash = 'ih-3'; inputs = @{ header = 'Key/4|c' } }
            ) }
    }
    '^ledger gate' { Emit @{ schema_version = 1; layer = 'models'; complete = $true; counts = @{ total = 3; done = 3; failed = 0; open = 0 } } }
    '^ledger hold' {
        $null = $flat -match ' -Units (\S+)'; $list = $Matches[1]
        Emit @{ schema_version = 1; held = @($list.Split(',') | ForEach-Object { $i = $_.LastIndexOf(':'); @{ unit = $_.Substring(0, $i); token = [int]$_.Substring($i + 1) } }); released = @(); renewals = 3; seconds = 0 }
    }
    '^finish-batch' {
        $null = $flat -match ' -Units (\S+)'; $list = $Matches[1]
        $results = @($list.Split(',') | ForEach-Object { $i = $_.LastIndexOf(':'); @{ unit = $_.Substring(0, $i); token = [int]$_.Substring($i + 1); status = 'done'; disposition = 'verified' } })
        Emit @{ schema_version = 1; kind = 'finish_batch'; units = $results.Count; done = $results.Count; results = $results }
    }
    '^list-models' {
        if ($env:MS_NO_LIST -eq '1') { Write-Output 'zt-digest list-models: the archive is unreachable'; exit 1 }
        Emit @{ ok = $true; models = @(
                @{ address = 'Key/2|c'; title = '模型一'; group = 1; level = 'formal'; form = $null; parts = 0; issues = 0; faults = 0 },
                @{ address = 'Key/3|c'; title = '模型二'; group = 2; level = 'concept'; form = 'theory'; parts = 0; issues = 0; faults = 0 },
                @{ address = 'Key/4|c'; title = '模型三'; group = 3; level = $null; form = $null; parts = 4; issues = 0; faults = 0 },
                @{ address = 'Key/5|c'; title = '模型四'; group = 4; level = 'formal'; form = $null; parts = 2; issues = 1; faults = 1 }
            ) }
    }
    '^model-deps' {
        if ($env:MS_GARBLED_DEPS -eq '1' -and $flat -match ' -Units ') { Write-Output 'model-deps registered every model (abridged)'; exit 0 }
        $null = $flat -match ' -Units (\S+)'; if ($Matches[1]) {
            $list = $Matches[1]
            $results = @($list.Split(',') | ForEach-Object {
                    $i = $_.LastIndexOf(':'); $unit = $_.Substring(0, $i)
                    $waiting = ($unit -eq 'models/g002-m0001@1')
                    @{ unit = $unit; ok = $true; waiting = $waiting; state = $(if ($waiting) { 'registered' } else { 'claimed' }) }
                })
            Emit @{ schema_version = 1; kind = 'model_deps_batch'; units = $results.Count; results = $results }
        }
        Emit @{ ok = $true; unit = 'models/g002-m0001@1'; waiting = $true; refs = @(); state = 'registered' }
    }
    '^model ' {
        if ($env:MS_FAIL -and $flat -match $env:MS_FAIL) { Write-Output "zt-digest model: the compiler refused"; exit 1 }
        if ($flat -match '-Step plan-status') {
            Emit @{ ok = $true; step = 'plan-status'; planned = 2; derive = $true; plans = @(
                    @{ model = 'Key/2|c'; status = 'planned'; level = 'formal'; parts = 4; derivations = 1; notes = @(); summary = 's' },
                    @{ model = 'Key/3|c'; status = 'planned'; level = 'concept'; parts = 2; derivations = 0 },
                    @{ model = 'Key/5|c'; status = 'planned'; level = 'formal'; parts = 3; derivations = 2 }
                ) }
        }
        if ($flat -match '-Step defer') { Emit @{ ok = $true; step = 'defer'; models = @('Key/3|c') } }
        if ($flat -match '-Step report') { Emit @{ ok = $true; step = 'report'; models = @(@{ model = 'Key/2|c'; status = 'done'; parts = 4; faults = @{} }) } }
        if ($flat -match '-Step close') {
            if ($env:MS_CLOSE_JOBS -eq '1') { Emit @{ ok = $true; step = 'close'; jobs = @(@{ id = 'Key_2_c:f01+9'; continue = 'Key_2_c:f01'; files = @('D:\x\follow.md') }); faults = 1; unowned = @(); report = $null } }
            Emit @{ ok = $true; step = 'close'; jobs = @(); faults = 0; unowned = @(); report = @{ models = @(@{ model = 'Key/2|c'; status = 'done'; parts = 4; faults = @{} }) } }
        }
        Emit @{ ok = $true; step = ($flat -replace '.*-Step ([a-z-]+).*', '$1'); jobs = @(@{ id = 'j1'; files = @(); prompt = 'p' }) }
    }
    '^term-cards' { Emit @{ ok = $true } }
}
Emit @{ ok = $true }
'@
Set-Content -LiteralPath $tool -Value $toolBody -Encoding utf8
$env:MS_CALLS = $calls

function Step([string[]]$argv) {
    $text = & pwsh -NoProfile -File $script @argv 2>&1
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { "$_".Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = if ($line) { $line | ConvertFrom-Json -Depth 16 } else { $null }
    [pscustomobject]@{ code = $code; json = $json; text = (@($text) -join ' | ') }
}
$common = @('-Out', $out, '-Tool', $tool)

# 1. start: the block is claimed, the block's models are named from the listing, and stopped claims are released first.
$env:MS_STALE = '1'
$start = Step (@('-Step', 'start', '-CitationKey', 'Key', '-Take', '8', '-Owner', 'models block 1') + $common)
$env:MS_STALE = ''
Assert ($start.json -and $start.json.ok -eq $true -and $start.json.claimed -eq 3 -and $start.json.units -eq 3 -and $start.json.models -eq 3 -and $start.json.batches -eq 2 -and $start.json.plan -eq 'concept:Key/3|c;formal:Key/2|c') "start: three Model units claimed and the block's three models named ($($start.text))"
$callsText = Get-Content -LiteralPath $calls -Raw
Assert ($callsText -match 'ledger status -Out .* -Layer models' -and $callsText -match 'ledger reclaim -Out .* -Unit models/g001-m0000@1 -Release') 'start: a stopped run''s claim is released before the new plan'
Assert ($callsText -match 'ledger next .* -Layer models -Take 8 -Owner models block 1 -Lease 7200 -Brief') 'start: one claim carries the owner and the two-hour lease'
Assert ($callsText -match 'list-models -Staging .* -All -CitationKey Key') 'start: the listing is the book''s (one command)'
$models = Get-Content -Raw (Join-Path $out 'models\drive\models.json') | ConvertFrom-Json
Assert (@($models).Count -eq 3 -and @($models | Where-Object { $_.address -eq 'Key/3|c' })[0].form -eq 'theory' -and @($models | Where-Object { $_.address -eq 'Key/4|c' })[0].parts -eq 4) 'start: each named model keeps its form, level, parts and issues, so the module can plan the batch'
$units = Get-Content -Raw (Join-Path $out 'models\drive\units.json') | ConvertFrom-Json
Assert (@($units).Count -eq 3 -and @($units)[0].id -eq 'models/g001-m0000@1') 'start: the claimed units are saved for the block''s steps'

# 2. brief: one compiler command, its jobs saved for the writers.
$brief = Step (@('-Step', 'brief', '-Work', $work, '-Models', 'Key/2|c,Key/5|c', '-MaxSentences', '10') + $common)
Assert ($brief.json -and $brief.json.ok -eq $true -and $brief.json.jobs -eq 1 -and $brief.json.file -like '*drive*jobs.json') "brief: the brief job is listed and saved ($($brief.text))"
$briefCalls = Get-Content -LiteralPath $calls -Raw
Assert ($briefCalls -match 'model -Step brief -Work .* -Staging .* -Models Key/2\|c,Key/5\|c -MaxSentences 10 ') 'brief: the models and the compile limits reach the one compiler command'
Assert ($briefCalls -match 'ledger hold .* -Units models/g001-m0000@1:1,models/g002-m0001@1:1,models/g003-m0002@1:1') 'brief: the step renews the block''s claims before it runs the chain'

# 3. fill: plan-status, one model-deps command, the defer, and the fill — in one step.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$fill = Step (@('-Step', 'fill', '-Work', $work, '-Units', 'models/g001-m0000@1:1,models/g002-m0001@1:1,models/g003-m0002@1:1') + $common)
$fillCalls = Get-Content -LiteralPath $calls -Raw
Assert ($fill.json -and $fill.json.ok -eq $true -and $fill.json.plans -eq 3) "fill: the plans the architect wrote are read ($($fill.text))"
Assert ($fill.json.deferred -eq 'Key/3|c:waiting') 'fill: the model whose producer is not done is released to wait'
Assert ($fill.json.derive -eq $true -and $fill.json.jobs -eq 1) 'fill: the derivation plans are read from the plans, and the fill jobs are listed'
Assert ($fillCalls -match 'model -Step plan-status ' -and $fillCalls -match 'model-deps -Out .* -Units models/g001-m0000@1:1,models/g002-m0001@1:1' -and $fillCalls -match 'model -Step defer -Work .* -Models Key/3\|c' -and $fillCalls -match 'model -Step apply -Work .* -MaxSentences 12 ') 'fill: plan-status, one model-deps command, the defer, then the fill — one process, one step'

# 4. derive, review, split (with a need), close (its report), report: each is one command.
$derive = Step (@('-Step', 'derive', '-Work', $work) + $common)
Assert ($derive.json -and $derive.json.ok -eq $true -and $derive.json.jobs -eq 1) "derive: the derivation job is listed ($($derive.text))"
$review = Step (@('-Step', 'review', '-Work', $work, '-Changed') + $common)
Assert ($review.json -and $review.json.ok -eq $true -and (Get-Content -LiteralPath $calls -Raw) -match 'model -Step review -Work .* -Changed') "review: -Changed reaches the one review command ($($review.text))"
$split = Step (@('-Step', 'split', '-Work', $work, '-Need', 'Key/2|c=3') + $common)
Assert ($split.json -and $split.json.ok -eq $true -and (Get-Content -LiteralPath $calls -Raw) -match 'model -Step split -Work .* -Need Key/2\|c=3') "split: the composers' needs reach the one split command ($($split.text))"
$env:MS_CLOSE_JOBS = '1'
$closeJobs = Step (@('-Step', 'close', '-Work', $work) + $common)
$env:MS_CLOSE_JOBS = ''
Assert ($closeJobs.json -and $closeJobs.json.ok -eq $true -and $closeJobs.json.jobs -eq 1 -and $closeJobs.json.faults -eq 1 -and -not $closeJobs.json.report) "close: a round that sends follow-ups lists them and writes no report ($($closeJobs.text))"
$close = Step (@('-Step', 'close', '-Work', $work) + $common)
Assert ($close.json -and $close.json.ok -eq $true -and -not $close.json.jobs -and $close.json.report_models -eq 1 -and $close.json.statuses -eq 'Key/2|c:done') "close: the round that sends nothing back carries the batch report ($($close.text))"
$report = Step (@('-Step', 'report', '-Work', $work) + $common)
Assert ($report.json -and $report.json.ok -eq $true -and @($report.json.models).Count -eq 1) "report: one report per model of the batch ($($report.text))"

# 5. a claim line that lost its units is read from the ledger; a model-deps batch line that did not come
# back whole is decided per unit by the ledger.
$out2 = Join-Path $scratch 'out2'
New-Item -ItemType Directory -Force $out2 | Out-Null
$env:MS_RENAMED = '1'
$renamed = Step (@('-Step', 'start', '-CitationKey', 'Key', '-Take', '8', '-Owner', 'models block 1', '-Out', $out2, '-Tool', $tool))
$env:MS_RENAMED = ''
Assert ($renamed.json -and $renamed.json.ok -eq $true -and $renamed.json.claimed -eq 1 -and $renamed.json.units -eq 1) "lost claim line: the held units come from the ledger ($($renamed.text))"
Assert ((Get-Content -LiteralPath $calls -Raw) -match 'ledger status -Out .* -Held models block 1') 'lost claim line: the fallback is the ledger read by owner'
$env:MS_GARBLED_DEPS = '1'
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$garbled = Step (@('-Step', 'fill', '-Work', $work, '-Units', 'models/g001-m0000@1:1', '-Out', $out2, '-Tool', $tool))
$env:MS_GARBLED_DEPS = ''
$garbledCalls = Get-Content -LiteralPath $calls -Raw
Assert ($garbled.json -and $garbled.json.ok -eq $true -and $garbled.json.jobs -eq 1) "garbled model-deps: the batch still fills ($($garbled.text))"
Assert ($garbledCalls -match 'ledger status -Out .* -Units models/g001-m0000@1' -and $garbledCalls -match 'model-deps -Out .* -Model Key/2\|c -Token 1') 'garbled model-deps: the ledger decides the unit and only a claimed one registers on its own'

# 6. finish: every named unit's checked completion in one command, then the gate.
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$finish = Step (@('-Step', 'finish', '-Units', 'models/g001-m0000@1:1,models/g002-m0001@1:1') + $common)
$finishCalls = Get-Content -LiteralPath $calls -Raw
Assert ($finish.json -and $finish.json.ok -eq $true -and $finish.json.done -eq 2 -and $finish.json.gate_complete -eq $true -and -not $finish.json.failed) "finish: both units complete and the gate settles ($($finish.text))"
Assert ($finishCalls -match 'finish-batch -Out .* -Units models/g001-m0000@1:1,models/g002-m0001@1:1' -and $finishCalls -notmatch 'model-finish ') 'finish: one finish-batch command runs each unit''s own verifier'

# 6. a step whose command fails is one ok:false line, never a half-written step.
$env:MS_FAIL = 'plan-status'
$bad = Step (@('-Step', 'fill', '-Work', $work, '-Units', 'models/g001-m0000@1:1') + $common)
$env:MS_FAIL = ''
Assert ($bad.json -and $bad.json.ok -eq $false -and $bad.json.step -eq 'plan-status') "fail: a failed command answers one ok:false line with its step ($($bad.text))"

# 7. no ready unit is an empty block, never an error; a failed listing is one failed step.
$env:MS_NO_UNITS = '1'
$empty = Step (@('-Step', 'start', '-CitationKey', 'Key', '-Take', '8', '-Owner', 'models block 2') + $common)
$env:MS_NO_UNITS = ''
Assert ($empty.json -and $empty.json.ok -eq $true -and -not $empty.json.claimed -and -not $empty.json.units -and $empty.json.gate_complete -eq $true) "empty claim: no ready unit is an empty success with the gate ($($empty.text))"
$env:MS_NO_LIST = '1'
$noList = Step (@('-Step', 'start', '-CitationKey', 'Key', '-Take', '8', '-Owner', 'models block 3') + $common)
$env:MS_NO_LIST = ''
Assert ($noList.json -and $noList.json.ok -eq $false -and $noList.json.step -eq 'list') "no listing: the start step stops with the listing's error ($($noList.text))"

# 8. #181: a start whose -CitationKey is empty falls back to the prepared run's manifest.json, so a resume's model
# listing is the book's Models, never every Model of the Workspace.
$manOut = Join-Path $scratch 'out-manifest'
New-Item -ItemType Directory -Force $manOut | Out-Null
[IO.File]::WriteAllText((Join-Path $manOut 'manifest.json'), '{"citation_key":"ManifestKey"}', [Text.UTF8Encoding]::new($false))
Remove-Item -LiteralPath $calls -ErrorAction SilentlyContinue
$manStart = Step (@('-Step', 'start', '-Take', '8', '-Owner', 'models block 4', '-Out', $manOut, '-Tool', $tool))
Assert ($manStart.json -and $manStart.json.ok -eq $true -and (Get-Content -LiteralPath $calls -Raw) -match 'list-models -Staging .* -All -CitationKey ManifestKey') "manifest key: the model listing names the prepared run's Citation key when the step names none ($($manStart.text))"

Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
Write-Host ''
Write-Host "model-steps: $ok passed, $failed failed"
if ($failed) { exit 1 } else { exit 0 }













