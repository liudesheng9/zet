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
        $nf = if ($env:CS_NORM) { $env:CS_NORM } else { 'D:\x\cited\normalize\jobs.json' }
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 35; unparsed = 2; jobs = 1; file = $nf; plan = 'D:\x\cited\plan.json' })
        exit 0
    }
    '^cited -Step apply' {
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 35; card = 'K/1|b' })
        exit 0
    }
    '^ledger status' {
        if ($flat -match '-Layer cited( |$)' -and $env:CS_CITED_STATE) { Write-Output (ConvertTo-Json -Compress -Depth 6 @{ schema_version = 1; units = @(@{ id = 'cited/book@1'; state = $env:CS_CITED_STATE }) }); exit 0 }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; units = @(@{ id = 'citedcards/t001@1'; state = 'claimed' }) })
        exit 0
    }
    '^ledger next' {
        if ($flat -match '-Lease (\d+)' -and [int]$Matches[1] -gt 7200) { Write-Output 'zt-digest ledger: -Lease must be a whole number of seconds from 1 through 7200'; exit 1 }
        if ($flat -match '-Owner cited cards finish') {
            $n = 0; if (Test-Path -LiteralPath $env:CS_STALE_NEXT) { $n = [int](Get-Content -Raw $env:CS_STALE_NEXT) }; Set-Content -LiteralPath $env:CS_STALE_NEXT -Value "$($n + 1)"
            if ($n -eq 0) { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'citedcards/t001@1'; token = 8; input_hash = 'ih-1'; inputs = @{ job = @{ id = 't001' } } }) }) } else { Write-Output (ConvertTo-Json -Compress @{ units = @() }) }
            exit 0
        }
        if ($flat -match '-Layer cited ') {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'cited/book@1'; token = 4; input_hash = 'ih-c' }) })
            exit 0
        }
        if ($flat -match '-Take 1 ') {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'citedcards/book@1'; token = 9; input_hash = 'ih-b' }) })
            exit 0
        }
        # v3: the start claims page after page until a page holds no job unit.
        $page = 1
        if ($flat -match '-Take 64') { if (Test-Path -LiteralPath $env:CS_NEXT_COUNT) { $page = [int](Get-Content -Raw $env:CS_NEXT_COUNT) + 1 }; Set-Content -LiteralPath $env:CS_NEXT_COUNT -Value "$page" }
        if ($page -eq 1) {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(
                        @{ id = 'citedcards/t001@1'; token = 1; input_hash = 'ih-1'; inputs = @{ job = @{ id = 't001' } } },
                        @{ id = 'citedcards/t002@1'; token = 2; input_hash = 'ih-2'; inputs = @{ job = @{ id = 't002' } } },
                        @{ id = 'citedcards/book@1'; token = 8; input_hash = 'ih-b' }) })
        } elseif ($page -eq 2 -and $env:CS_TWO_PAGES) {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'citedcards/t003@1'; token = 3; input_hash = 'ih-3'; inputs = @{ job = @{ id = 't003' } } }) })
        } else { Write-Output (ConvertTo-Json -Compress @{ units = @() }) }
        exit 0
    }
    '^cited-cards -Step plan' {
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; jobs = 2; revision = 1; rows = 8 })
        exit 0
    }
    '^cited-cards -Step jobs' {
        $all = @(@{ id = 't001'; files = @('a.md', 'b.md'); prompt = 'p' }, @{ id = 't002'; files = @('a.md', 'c.md'); prompt = 'p' }, @{ id = 't003'; files = @('a.md', 'd.md'); prompt = 'p' })
        $wanted = @(([regex]::Matches($flat, 't\d{3}(?=:)')) | ForEach-Object { $_.Value })
        if (-not $wanted.Count) { $wanted = @('t001', 't002') }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @($all | Where-Object { $wanted -contains $_.id }) })
        exit 0
    }
    '^cited-cards -Step check' {
        if ($env:CS_SETTLE) {
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $env:CS_SETTLE) | Out-Null
            ConvertTo-Json -Depth 6 @{ kind = 'cited_cards_readiness'; rows = @(@{ row = 1; job = 't001'; status = 'ready' }, @{ row = 2; job = 't001'; status = 'ready' }, @{ row = 3; job = 't002'; status = 'ready' }, @{ row = 4; job = 't002'; status = 'not_ready' }) } | Set-Content -LiteralPath $env:CS_SETTLE -Encoding utf8
        }
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; ready = 7; not_ready = 1 }); exit 0
    }
    '^ledger done' {
        if ($env:CS_NOT_READY -and $flat -match 'citedcards/t002@1') { Write-Output 'zt-digest ledger: row 8 is faults, not ready'; exit 1 }
        if ($env:CS_STALE -and $flat -match 'citedcards/t001@1 -Token 1 ') { Write-Output 'zt-digest ledger: stale_claim: citedcards/t001@1 has no current unexpired claim for token 1'; exit 1 }
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
$env:CS_NEXT_COUNT = Join-Path $scratch 'next-page.txt'
$env:CS_STALE_NEXT = Join-Path $scratch 'stale-next.txt'

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
# v3: the normalize jobs are cut into one chunk file per manager too.
$norm = Join-Path $scratch 'normalize-jobs.json'
ConvertTo-Json -Depth 4 @(@{ id = 'n1' }, @{ id = 'n2' }, @{ id = 'n3' }) | Set-Content -LiteralPath $norm -Encoding utf8
$env:CS_NORM = $norm
$nl = Step (@('-Step', 'list', '-Chunk', '2') + $common)
$env:CS_NORM = ''
$n1 = @(Get-Content -Raw (Join-Path $out 'cited\drive\normalize-001.json') | ConvertFrom-Json)
$n2 = @(Get-Content -Raw (Join-Path $out 'cited\drive\normalize-002.json') | ConvertFrom-Json)
Assert ($nl.json.chunks -eq 2 -and ($n1.id -join ',') -eq 'n1,n2' -and ($n2.id -join ',') -eq 'n3') "list v3: the normalize jobs are cut into chunk files ($($nl.text))"
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
# A resume over a finished list plans nothing (scheiner2022baghdad, 2026-11-04).
$env:CS_CITED_STATE = 'done'
$skip = Step (@('-Step', 'list', '-SkipDone') + $common)
Assert ($skip.json.ok -eq $true -and $skip.json.settled -eq $true) "list -SkipDone: a done unit answers settled ($($skip.text))"
Assert ((Get-Content -LiteralPath $calls -Raw) -notmatch 'cited -Step plan') 'list -SkipDone: a done unit plans nothing'
$env:CS_CITED_STATE = 'retry'
Remove-Item -LiteralPath $calls
$open = Step (@('-Step', 'list', '-SkipDone') + $common)
Assert (-not $open.json.settled -and (Get-Content -LiteralPath $calls -Raw) -match 'cited -Step plan') 'list -SkipDone: an open unit plans as usual'
$env:CS_CITED_STATE = ''
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
Assert (@(Get-Content -LiteralPath $calls | Where-Object { $_ -match '^ledger next .*-Layer citedcards .*-Take 64' }).Count -eq 2 -and $c -notmatch '-Lease (2[0-9]{4}|[89][0-9]{3})') 'start v3: it claims until a page holds no job unit, under the ledger''s lease limit'
$chunk1 = @(Get-Content -Raw (Join-Path $out 'cited\drive\jobs-001.json') | ConvertFrom-Json)
Assert ($start.json.chunks -eq 1 -and ($chunk1.id -join ',') -eq 't001,t002') "start v3: one chunk file per manager holds its jobs in order ($($start.text))"
# v3 (atkinson2015lectures, 2026-11-02): one start claimed at most 64 jobs; the start now claims every page.
Remove-Item -LiteralPath $env:CS_NEXT_COUNT -ErrorAction SilentlyContinue
$env:CS_TWO_PAGES = '1'
$three = Step (@('-Step', 'start', '-Chunk', '2') + $common)
$env:CS_TWO_PAGES = ''
$chunk1 = @(Get-Content -Raw (Join-Path $out 'cited\drive\jobs-001.json') | ConvertFrom-Json)
$chunk2 = @(Get-Content -Raw (Join-Path $out 'cited\drive\jobs-002.json') | ConvertFrom-Json)
Assert ($three.json.jobs -eq 3 -and $three.json.chunks -eq 2 -and ($chunk1.id -join ',') -eq 't001,t002' -and ($chunk2.id -join ',') -eq 't003') "start v3: two pages list three jobs in chunks of two and one ($($three.text))"
Remove-Item -LiteralPath $env:CS_NEXT_COUNT -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $calls
$null = Step (@('-Step', 'start') + $common)
Remove-Item -LiteralPath $env:CS_NEXT_COUNT -ErrorAction SilentlyContinue
# A restarted branch (atkinson2015lectures, 2026-11-02: the DeepSeek Desktop stalled after 267 of 398 drafts) keeps its
# drafts: a claimed job whose rows are all ready completes at start and is never written again.
Remove-Item -LiteralPath $calls
$env:CS_SETTLE = Join-Path $out 'cited-cards\check\readiness.json'
$settle = Step (@('-Step', 'start') + $common)
Remove-Item -LiteralPath $env:CS_SETTLE -ErrorAction SilentlyContinue
$env:CS_SETTLE = ''
Remove-Item -LiteralPath $env:CS_NEXT_COUNT -ErrorAction SilentlyContinue
$c = Get-Content -LiteralPath $calls -Raw
Assert ($settle.json.settled -eq 1 -and $settle.json.jobs -eq 1 -and $c -match 'ledger done .*-Unit citedcards/t001@1 -Token 1 -Outputs .*readiness\.json' -and $c -match 'cited-cards -Step jobs -Staging .* -Tokens t002:2' -and $c -notmatch 'Tokens t001') "start: a job whose drafts are all ready settles and only the rest are written ($($settle.text))"
Remove-Item -LiteralPath $calls
$null = Step (@('-Step', 'start') + $common)
Remove-Item -LiteralPath $env:CS_NEXT_COUNT -ErrorAction SilentlyContinue
# v3: a job that outlasts its claim's lease is taken back and completed under a new token.
$env:CS_STALE = '1'
Remove-Item -LiteralPath $calls
$late = Step (@('-Step', 'finish') + $common)
$env:CS_STALE = ''
$lc = @(Get-Content -LiteralPath $calls)
Assert ($late.json.done -eq 2 -and -not $late.json.failed -and @($lc | Where-Object { $_ -match '^ledger reclaim .*-Unit citedcards/t001@1$' }).Count -eq 1 -and @($lc | Where-Object { $_ -match '^ledger done .*-Unit citedcards/t001@1 -Token 8 ' }).Count -eq 1) "finish v3: an expired claim is taken back and the job completes ($($late.text))"
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
