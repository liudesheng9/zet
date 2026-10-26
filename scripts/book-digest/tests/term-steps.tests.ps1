#Requires -Version 7.5
<#
.SYNOPSIS
  The Term stages' deterministic steps (term-steps.ps1, #172, #180): one script call runs a whole chain and prints
  one compact JSON line of at most 400 characters whose top-level `ok` is a boolean, with the full data beside it
  in the detail file the line names.

.DESCRIPTION
  A stand-in zt-digest answers the tool's commands from a small table, so the suite proves the step's own
  behaviour without an archive, a model or a provider:
  - `start` releases a stopped run's claims, plans, claims every ready job unit and lists the claimed jobs;
  - `review` and `fix` list their jobs and keep the plan's counts out of the line;
  - `finish` completes each claimed job from the readiness result, publishes the book and answers counts;
  - `link` plans, applies under the claim and plans again, answering counts, never a nested object;
  - a failed command answers one `{ok:false, step, error}` line of its own;
  - every answer is at most 400 characters, has a boolean top-level `ok` and names its detail file.
#>
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$here = $PSScriptRoot
$script = Join-Path (Split-Path $here -Parent) 'term-steps.ps1'
$failed = 0
$ok = 0
function Assert([bool]$condition, [string]$name) {
    if ($condition) { $script:ok++; Write-Host "ok: $name" } else { $script:failed++; Write-Host "FAIL: $name" }
}
$scratch = Join-Path ([IO.Path]::GetTempPath()) "term-steps-test-$PID"
if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch }
$tool = Join-Path $scratch 'fake-zt-digest.ps1'
$out = Join-Path $scratch 'out'
New-Item -ItemType Directory -Force $out | Out-Null
$calls = Join-Path $scratch 'calls.txt'
$toolBody = @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$ErrorActionPreference = 'Stop'
$args2 = $Rest
Add-Content -LiteralPath $env:TS_CALLS -Value ($args2 -join ' ') -Encoding utf8
$flat = $args2 -join ' '
$flat = $args2 -join ' '
switch -Regex ($flat) {
    '^ledger status' {
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ schema_version = 1; units = @(@{ id = 'termcards/t001@1'; state = 'claimed' }) })
        exit 0
    }
    '^term-cards -Step plan' {
        if ($env:TS_PLAN_FAIL) { Write-Output 'zt-digest term-cards: the book has no Term list'; exit 1 }
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; step = 'plan'; revision = 3; rows = 2 })
        exit 0
    }
    '^ledger next' {
        if ($flat -match '-Layer termlink') {
            if ($env:TS_NO_LINK_UNIT) { Write-Output (ConvertTo-Json -Compress @{ units = @() }); exit 0 }
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'termlink/book@1'; token = 5; input_hash = 'ih-l' }) })
            exit 0
        }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(
                    @{ id = 'termcards/t001@1'; token = 1; input_hash = 'ih-1'; inputs = @{ job = @{ id = 't001' } } },
                    @{ id = 'termcards/t002@1'; token = 2; input_hash = 'ih-2'; inputs = @{ job = @{ id = 't002' } } },
                    @{ id = 'termcards/book@1'; token = 9; input_hash = 'ih-b' }) })
        exit 0
    }
    '^term-cards -Step jobs' {
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @(@{ id = 't001'; files = @(); prompt = 'p' }, @{ id = 't002'; files = @(); prompt = 'p' }) })
        exit 0
    }
    '^term-cards -Step check' { Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 2 }); exit 0 }
    '^term-cards -Step review-plan' {
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @(@{ id = 'r1'; delete_after_collect = $true }, @{ id = 'r2'; delete_after_collect = $false }); counts = @{ rows = 2; needs_review = 1 } })
        exit 0
    }
    '^term-cards -Step fix-plan' {
        if ($env:TS_FIX_FAIL) { Write-Output 'no answer file'; exit 1 }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @(@{ id = 'f1'; continue = 't001'; files = @('x'); fallback = @('a', 'b') }); counts = @{ fixes = 1 } })
        exit 0
    }
    '^ledger done' {
        if ($env:TS_DONE_REFUSE -and $flat -match 'termcards/book@1') { Write-Output 'zt-digest ledger: the book is not ready'; exit 1 }
        Write-Output (ConvertTo-Json -Compress @{ schema_version = 1; state = 'done' })
        exit 0
    }
    '^term-cards -Step apply' {
        if ($env:TS_APPLY_STALE) { Write-Output (ConvertTo-Json -Compress @{ ok = $false; step = 'apply'; code = 'plan_stale'; hint = 'run -Step plan again' }); Write-Output 'zt-digest term-cards: plan_stale: the frozen plan no longer describes this archive'; exit 1 }
        if ($env:TS_APPLY_ERROR) { Write-Output 'zt-digest term-cards: the publication was refused: row 1 has no draft'; exit 1 }
        if ($env:TS_PARTIAL) { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; complete = $false; rows = 2; committed = 1; ready_unpublished = 1; stop = @{ code = 'not_ready' } }); exit 0 }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; complete = $true; rows = 2; committed = 2; ready_unpublished = 0; stop = $null })
        exit 0
    }
    '^ledger gate' { Write-Output (ConvertTo-Json -Compress @{ complete = $true }); exit 0 }
    '^term-link -Step plan' {
        if ($env:TS_LINK_PLAN_FAIL) { Write-Output 'zt-digest term-link: the book has no Term list to link'; exit 1 }
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; plan_revision = 2; cards_changed = 1; cards_refused = 0 })
        exit 0
    }
    '^term-link -Step apply' {
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ step = 'apply'; complete = $true; no_op = $false; counts = @{ cards_changed = 1; links_added = 2 }; report = 'apply.json' })
        exit 0
    }
}
Write-Output (ConvertTo-Json -Compress @{ ok = $true })
exit 0
'@
Set-Content -LiteralPath $tool -Value $toolBody -Encoding utf8
$env:TS_CALLS = $calls
$env:TS_OUT = $out

# One step: its exit code, the last non-empty line and that line parsed.
function Step([string[]]$argv) {
    $text = & pwsh -NoProfile -File $script @argv 2>&1
    $code = $LASTEXITCODE
    $lines = @($text | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
    $last = if ($lines.Count) { $lines[-1] } else { '' }
    $json = $null
    if ($last.StartsWith('{')) { try { $json = $last | ConvertFrom-Json -Depth 16 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; last = $last; json = $json; text = (@($text) -join ' | ') }
}
# #180: every step of the script answers one compact line: at most 400 characters, a boolean top-level `ok`, and
# the detail file it names exists.
function Assert-Line([object]$r, [string]$name) {
    Assert ($r.last.Length -gt 0 -and $r.last.Length -le 400) "compact ${name}: the last line is at most 400 characters ($($r.last.Length))"
    Assert ($r.json -and $r.json.ok -is [bool]) "compact ${name}: the last line has a boolean top-level ok ($($r.last))"
    Assert ($r.json -and $r.json.detail -and (Test-Path -LiteralPath $r.json.detail)) "compact ${name}: the line names its detail file and it exists"
}
$common = @('-Out', $out, '-Tool', $tool)

# 1. start: the plan runs, every ready job unit is claimed, its two jobs are listed.
$start = Step (@('-Step', 'start') + $common)
Assert-Line $start 'start'
Assert ($start.json.ok -eq $true -and $start.json.jobs -eq 2 -and $start.json.plan_revision -eq 3 -and $start.json.file) "start: two jobs listed on plan revision 3 ($($start.text))"
$startDetail = Get-Content -LiteralPath $start.json.detail -Raw | ConvertFrom-Json
Assert (@($startDetail.detail.units).Count -eq 2 -and @($startDetail.detail.jobs).Count -eq 2) 'start: the detail file keeps the claimed units and the job list'
$startCalls = Get-Content -LiteralPath $calls -Raw
Assert ($startCalls -match 'ledger reclaim .*-Unit termcards/t001@1 -Release') 'start: a stopped run''s claim is released before the plan'

# 2. review and fix list their jobs, and their counts stay out of the line.
$review = Step (@('-Step', 'review') + $common)
Assert-Line $review 'review'
Assert ($review.json.ok -eq $true -and $review.json.reviews -eq 1 -and -not $review.json.PSObject.Properties['counts']) "review: one review job, no nested counts in the line ($($review.text))"
$fix = Step (@('-Step', 'fix') + $common)
Assert-Line $fix 'fix'
Assert ($fix.json.ok -eq $true -and $fix.json.fixes -eq 1) "fix: one fix job ($($fix.text))"
$fixes = Get-Content -Raw (Join-Path $out 'term-cards\drive\fixes.json') | ConvertFrom-Json
Assert (@($fixes).Count -eq 1 -and @($fixes)[0].files[0] -eq 'a' -and -not @($fixes)[0].PSObject.Properties['continue']) 'fix: a fix runs from its fallback files and keeps no continue'

# 3. finish: each claimed job completes from the readiness result and the book publishes; the line answers counts.
$finish = Step (@('-Step', 'finish') + $common)
Assert-Line $finish 'finish'
Assert ($finish.json.ok -eq $true -and $finish.json.done -eq 2 -and -not $finish.json.failed -and $finish.json.book -eq 'done') "finish: both jobs done and the book published ($($finish.text))"
Assert ($finish.json.complete -eq $true -and $finish.json.rows -eq 2 -and $finish.json.committed -eq 2) 'finish: the publication answers flat counts, never a nested object'
$finishDetail = Get-Content -LiteralPath $finish.json.detail -Raw | ConvertFrom-Json
Assert (@($finishDetail.detail.done).Count -eq 2 -and $finishDetail.detail.publication.complete -eq $true) 'finish: the detail file keeps the done job ids and the publication'

# 4. a job that is not ready fails only its own unit; the ready prefix still publishes.
$env:TS_PARTIAL = '1'
$partial = Step (@('-Step', 'finish') + $common)
$env:TS_PARTIAL = ''
Assert-Line $partial 'finish partial'
Assert ($partial.json.ok -eq $true -and $partial.json.book -eq 'failed' -and -not $partial.json.complete -and $partial.json.committed -eq 1) "finish: an incomplete publication answers its prefix counts ($($partial.text))"

# Live run 2026-10-26 (staging5): the publication printed no line and the finish answered rows 0 with no reason.
# An apply that fails is named in the line.
$env:TS_APPLY_ERROR = '1'
$noPub = Step (@('-Step', 'finish') + $common)
$env:TS_APPLY_ERROR = ''
Assert-Line $noPub 'finish no publication'
Assert ($noPub.json.ok -eq $true -and $noPub.json.publish_error -match 'row 1 has no draft') "finish: a publication that prints no line names its error ($($noPub.text))"

# Live run 2026-10-26 (staging6): the apply answered {ok:false, code:plan_stale} and the finish named nothing.
$env:TS_APPLY_STALE = '1'
$stale = Step (@('-Step', 'finish') + $common)
$env:TS_APPLY_STALE = ''
Assert ($stale.json.ok -eq $true -and $stale.json.publish_error -match 'plan_stale') "finish: a refused publication names its code ($($stale.text))"

# 5. link: plan, apply under the claim, plan again; the line answers counts, never the nested plan.
$link = Step (@('-Step', 'link') + $common)
Assert-Line $link 'link'
Assert ($link.json.ok -eq $true -and $link.json.status -eq 'done' -and $link.json.unit -eq 'termlink/book@1' -and $link.json.cards_changed -eq 1 -and $link.json.links_added -eq 2) "link: the applied pass answers its own counts ($($link.text))"
Assert ($link.json.second_cards_changed -eq 1 -and -not $link.json.second_cards_refused) 'link: the second plan''s counts are flat fields'
Assert (-not $link.json.PSObject.Properties['counts'] -and -not $link.json.PSObject.Properties['second']) 'link: no nested counts or plan object is in the line'

# 6. no ready unit: the link pass is skipped with its reason, never a failed line.
$env:TS_NO_LINK_UNIT = '1'
$skipped = Step (@('-Step', 'link') + $common)
$env:TS_NO_LINK_UNIT = ''
Assert-Line $skipped 'link skipped'
Assert ($skipped.json.ok -eq $true -and $skipped.json.status -eq 'skipped' -and $skipped.json.reason) "link: an already complete pass answers skipped with its reason ($($skipped.text))"

# 7. a failed command answers one compact `{ok:false, step, error}` line.
$env:TS_PLAN_FAIL = '1'
$planFail = Step (@('-Step', 'start') + $common)
$env:TS_PLAN_FAIL = ''
Assert-Line $planFail 'start plan failed'
Assert ($planFail.code -eq 0 -and $planFail.json.ok -eq $false -and $planFail.json.step -eq 'plan' -and $planFail.json.error) "start: a failed plan is one ok:false line and exit 0 ($($planFail.text))"
$env:TS_LINK_PLAN_FAIL = '1'
$linkFail = Step (@('-Step', 'link') + $common)
$env:TS_LINK_PLAN_FAIL = ''
Assert-Line $linkFail 'link plan failed'
Assert ($linkFail.json.ok -eq $false -and $linkFail.json.step -eq 'plan') 'link: a failed plan is one ok:false line'

# 8. the detail file of every step lives under the stage folder and is named in the line.
$detailDir = Join-Path $out 'term-cards\drive-lines'
Assert ((Test-Path -LiteralPath (Join-Path $detailDir 'start.json')) -and (Test-Path -LiteralPath (Join-Path $detailDir 'link.json'))) 'detail: each step writes its own detail file under the stage folder'

Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
Write-Host ''
Write-Host "term-steps: $ok passed, $failed failed"
if ($failed) { exit 1 } else { exit 0 }



