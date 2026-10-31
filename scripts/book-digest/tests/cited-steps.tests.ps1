#Requires -Version 7.5
<#
.SYNOPSIS
  The Cited stages' deterministic steps (cited-steps.ps1, #221): one script call runs a whole chain and prints
  one compact JSON line of at most 400 characters whose top-level `ok` is a boolean, with the full data beside it
  in the detail file the line names.

.DESCRIPTION
  A stand-in zt-digest answers the tool's commands from a small table, so the suite proves the step's own
  behaviour without an archive, a model or a provider:
  - `list` plans the Cited list and names the normalize jobs;
  - `list-finish` claims cited/book@1, applies under the claim and completes it, or applies without a unit;
  - `start` releases a stopped run's claims, plans, claims every ready job unit and lists the claimed jobs;
  - `finish` completes each claimed job from the readiness result, publishes the book and answers counts;
  - a failed command answers one `{ok:false, step, error}` line of its own;
  - every answer is at most 400 characters, has a boolean top-level `ok` and names its detail file.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$script = Join-Path (Split-Path $here -Parent) 'cited-steps.ps1'
$failed = 0
$ok = 0
function Assert([bool]$condition, [string]$name) {
    if ($condition) { $script:ok++; Write-Host "ok: $name" } else { $script:failed++; Write-Host "FAIL: $name" }
}
$scratch = Join-Path ([IO.Path]::GetTempPath()) "cited-steps-test-$PID"
if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch }
$tool = Join-Path $scratch 'fake-zt-digest.ps1'
$out = Join-Path $scratch 'out'
New-Item -ItemType Directory -Force $out | Out-Null
$calls = Join-Path $scratch 'calls.txt'
$toolBody = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$ErrorActionPreference = 'Stop'
Add-Content -LiteralPath $env:CS_CALLS -Value ($Rest -join ' ') -Encoding utf8
$flat = $Rest -join ' '
switch -Regex ($flat) {
    '^cited -Step plan' {
        if ($env:CS_PLAN_FAIL) { Write-Output 'zt-digest cited: the staging manifest names no source'; exit 1 }
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 35; unparsed = 2; jobs = 1; file = 'D:\x\cited\normalize\jobs.json'; plan = 'D:\x\cited\plan.json' })
        exit 0
    }
    '^cited -Step apply' {
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 35; card = 'K/1|b' })
        exit 0
    }
    '^ledger status' {
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; units = @(@{ id = 'citedcards/t001@1'; state = 'claimed' }) })
        exit 0
    }
    '^ledger next' {
        if ($flat -match '-Layer cited ') {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'cited/book@1'; token = 4; input_hash = 'ih-c' }) })
            exit 0
        }
        if ($flat -match '-Take 1 ') {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'citedcards/book@1'; token = 9; input_hash = 'ih-b' }) })
            exit 0
        }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(
                    @{ id = 'citedcards/t001@1'; token = 1; input_hash = 'ih-1'; inputs = @{ job = @{ id = 't001' } } },
                    @{ id = 'citedcards/t002@1'; token = 2; input_hash = 'ih-2'; inputs = @{ job = @{ id = 't002' } } },
                    @{ id = 'citedcards/book@1'; token = 8; input_hash = 'ih-b' }) })
        exit 0
    }
    '^cited-cards -Step plan' {
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; jobs = 2; revision = 1; rows = 8 })
        exit 0
    }
    '^cited-cards -Step jobs' {
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @(@{ id = 't001'; files = @('a.md', 'b.md'); prompt = 'p' }, @{ id = 't002'; files = @('a.md', 'c.md'); prompt = 'p' }) })
        exit 0
    }
    '^cited-cards -Step check' { Write-Output (ConvertTo-Json -Compress @{ ok = $true; ready = 7; not_ready = 1 }); exit 0 }
    '^ledger done' {
        if ($env:CS_NOT_READY -and $flat -match 'citedcards/t002@1') { Write-Output 'zt-digest ledger: row 8 is faults, not ready'; exit 1 }
        Write-Output (ConvertTo-Json -Compress @{ schema_version = 1; state = 'done' })
        exit 0
    }
    '^cited-cards -Step apply' {
        if ($env:CS_PARTIAL) { Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 8; committed = 7 }); exit 0 }
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 8; committed = 8; complete = $true })
        exit 0
    }
}
Write-Output (ConvertTo-Json -Compress @{ ok = $true })
exit 0
'@
Set-Content -LiteralPath $tool -Value $toolBody -Encoding utf8
$env:CS_CALLS = $calls

function Step([string[]]$argv) {
    $text = & pwsh -NoProfile -File $script @argv 2>&1
    $code = $LASTEXITCODE
    $lines = @($text | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
    $last = if ($lines.Count) { $lines[-1] } else { '' }
    $json = $null
    if ($last.StartsWith('{')) { try { $json = $last | ConvertFrom-Json -Depth 16 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; last = $last; json = $json; text = (@($text) -join ' | ') }
}
function Assert-Line([object]$r, [string]$name) {
    Assert ($r.last.Length -gt 0 -and $r.last.Length -le 400) "compact ${name}: the last line is at most 400 characters ($($r.last.Length))"
    Assert ($r.json -and $r.json.ok -is [bool]) "compact ${name}: the last line has a boolean top-level ok ($($r.last))"
    Assert ($r.json -and $r.json.detail -and (Test-Path -LiteralPath $r.json.detail)) "compact ${name}: the line names its detail file and it exists"
}
$common = @('-Out', $out, '-Tool', $tool)

# 1. Without a ledger: list, list-finish without a unit, start lists every job, finish publishes without a claim.
$list = Step (@('-Step', 'list', '-Heading', 'Bibliography') + $common)
Assert-Line $list 'list'
Assert ($list.json.ok -eq $true -and $list.json.rows -eq 35 -and $list.json.unparsed -eq 2 -and $list.json.jobs -eq 1 -and $list.json.file) "list: rows, unparsed entries and the normalize job file ($($list.text))"
Assert ((Get-Content -LiteralPath $calls -Raw) -match 'cited -Step plan -Staging .* -Heading Bibliography') 'list: the heading reaches the plan'
$withSource = Step (@('-Step', 'list', '-Source', 'C:\books\whole.md') + $common)
Assert ($withSource.json.ok -eq $true -and ((Get-Content -LiteralPath $calls -Raw) -match 'cited -Step plan -Staging .* -Source C:\\books\\whole\.md')) 'list: -Source reaches the plan'
$lf = Step (@('-Step', 'list-finish') + $common)
Assert-Line $lf 'list-finish'
Assert ($lf.json.card -eq 'K/1|b' -and $lf.json.unit -eq 'unclaimed') "list-finish: no ledger, the list is written without a unit ($($lf.text))"
Assert ((Get-Content -LiteralPath $calls -Raw) -notmatch 'ledger next') 'list-finish: no ledger, no claim'
$start = Step (@('-Step', 'start', '-PerJob', '3') + $common)
Assert-Line $start 'start (no ledger)'
Assert ($start.json.jobs -eq 2 -and $start.json.plan_revision -eq 1 -and (Test-Path -LiteralPath $start.json.file)) "start: every job listed without a ledger ($($start.text))"
Assert ((Get-Content -LiteralPath $calls -Raw) -match 'cited-cards -Step plan -Staging .* -PerJob 3') 'start: -PerJob reaches the plan'
$finish = Step (@('-Step', 'finish') + $common)
Assert-Line $finish 'finish (no ledger)'
Assert ($finish.json.complete -eq $true -and $finish.json.committed -eq 8 -and $finish.json.book -eq 'done') "finish: no ledger, the publication is complete ($($finish.text))"

# 2. With a ledger: list-finish claims and completes cited/book@1; start releases, claims and lists; finish completes.
Set-Content -LiteralPath (Join-Path $out 'ledger.sqlite3') -Value '' -Encoding utf8
Remove-Item -LiteralPath $calls
$lf = Step (@('-Step', 'list-finish') + $common)
Assert ($lf.json.unit -eq 'done') "list-finish: the claimed unit completes ($($lf.text))"
$c = Get-Content -LiteralPath $calls -Raw
Assert ($c -match 'cited -Step apply -Staging .* -Token 4 -Apply' -and $c -match 'ledger done .*-Unit cited/book@1 -Token 4 -Outputs .*cited\\apply\.json') 'list-finish: apply under the claim, then done from the apply report'
Remove-Item -LiteralPath $calls
$start = Step (@('-Step', 'start') + $common)
Assert ($start.json.jobs -eq 2) "start: the claimed jobs are listed ($($start.text))"
$c = Get-Content -LiteralPath $calls -Raw
Assert ($c -match 'ledger reclaim .*-Unit citedcards/t001@1 -Release') 'start: a stopped run''s claim is released before the plan'
Assert ($c -match 'ledger reclaim .*-Unit citedcards/book@1 -Release') 'start: the book unit start hands out is released'
Assert ($c -match 'cited-cards -Step jobs -Staging .* -Tokens t001:1,t002:2') 'start: the jobs step names the claimed tokens'
$env:CS_NOT_READY = '1'; $env:CS_PARTIAL = '1'
Remove-Item -LiteralPath $calls
$finish = Step (@('-Step', 'finish') + $common)
Assert-Line $finish 'finish (partial)'
Assert ($finish.json.done -eq 1 -and $finish.json.failed -eq 1 -and $finish.json.book -eq 'failed' -and -not $finish.json.complete) "finish: a job not ready fails alone and the book stays open ($($finish.text))"
$c = Get-Content -LiteralPath $calls -Raw
Assert ($c -match 'ledger fail .*-Unit citedcards/t002@1') 'finish: the job that is not ready is failed in the ledger'
Remove-Item Env:CS_NOT_READY; Remove-Item Env:CS_PARTIAL
Remove-Item -LiteralPath $calls
$finish = Step (@('-Step', 'finish') + $common)
Assert ($finish.json.book -eq 'done' -and $finish.json.complete -eq $true) "finish: a complete publication completes the book unit ($($finish.text))"
Assert ((Get-Content -LiteralPath $calls -Raw) -match 'ledger done .*-Unit citedcards/book@1 -Token 9 -Outputs .*cited-cards\\report\.json') 'finish: the book completes from the report'

# 3. A failed command answers its own line.
$env:CS_PLAN_FAIL = '1'
$bad = Step (@('-Step', 'list') + $common)
Assert-Line $bad 'list (failed)'
Assert ($bad.json.ok -eq $false -and $bad.json.step -eq 'plan' -and $bad.json.error -match 'names no source') "a failed plan answers ok:false with its reason ($($bad.text))"
Remove-Item Env:CS_PLAN_FAIL

Remove-Item -Recurse -Force $scratch
Write-Host "cited-steps: $ok ok, $failed failed"
if ($failed) { exit 1 }
