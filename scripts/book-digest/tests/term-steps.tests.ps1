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
        if ($flat -match '-Lease (\d+)' -and [int]$Matches[1] -gt 7200) { Write-Output 'zt-digest ledger: -Lease must be a whole number of seconds from 1 through 7200'; exit 1 }
        # v10: a claim the finish takes back after its lease ran out comes back with a new token.
        if ($flat -match "-Owner term cards finish") {
            $n = 0; if (Test-Path -LiteralPath $env:TS_STALE_NEXT) { $n = [int](Get-Content -Raw $env:TS_STALE_NEXT) }; Set-Content -LiteralPath $env:TS_STALE_NEXT -Value "$($n + 1)"
            if ($n -eq 0) { Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'termcards/t001@1'; token = 8; input_hash = 'ih-1'; inputs = @{ job = @{ id = 't001' } } }) }) } else { Write-Output (ConvertTo-Json -Compress @{ units = @() }) }
            exit 0
        }
        if ($flat -match '-Layer termlink') {
            if ($env:TS_NO_LINK_UNIT) { Write-Output (ConvertTo-Json -Compress @{ units = @() }); exit 0 }
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'termlink/book@1'; token = 5; input_hash = 'ih-l' }) })
            exit 0
        }
        # v10: the start claims page after page (at most 64 units each) until a page has no job unit.
        $page = 1
        if ($env:TS_NEXT_COUNT -and $flat -match '-Take 64') { if (Test-Path -LiteralPath $env:TS_NEXT_COUNT) { $page = [int](Get-Content -Raw $env:TS_NEXT_COUNT) + 1 }; Set-Content -LiteralPath $env:TS_NEXT_COUNT -Value "$page" }
        if ($page -eq 1) {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(
                        @{ id = 'termcards/t001@1'; token = 1; input_hash = 'ih-1'; inputs = @{ job = @{ id = 't001' } } },
                        @{ id = 'termcards/t002@1'; token = 2; input_hash = 'ih-2'; inputs = @{ job = @{ id = 't002' } } },
                        @{ id = 'termcards/book@1'; token = 9; input_hash = 'ih-b' }) })
        } elseif ($page -eq 2 -and $env:TS_TWO_PAGES) {
            Write-Output (ConvertTo-Json -Compress -Depth 8 @{ units = @(@{ id = 'termcards/t003@1'; token = 3; input_hash = 'ih-3'; inputs = @{ job = @{ id = 't003' } } }) })
        } else { Write-Output (ConvertTo-Json -Compress @{ units = @() }) }
        exit 0
    }
    '^term-cards -Step jobs' {
        $all = @(@{ id = 't001'; kind = 'concept'; rows = @(1); files = @('instr.md', 'jobs\t001.md'); prompt = 'p' }, @{ id = 't002'; kind = 'math'; rows = @(2); files = @('instr.md', 'jobs\t002.md'); prompt = 'p' }, @{ id = 't003'; kind = 'concept'; rows = @(3); files = @('instr.md', 'jobs\t003.md'); prompt = 'p' })
        $wanted = @(([regex]::Matches($flat, 't\d{3}(?=:)')) | ForEach-Object { $_.Value })
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @($all | Where-Object { $wanted -contains $_.id }) })
        exit 0
    }
    '^term-cards -Step check' {
        # v10: with TS_SETTLE the check finds job t001's row already ready (its draft is on disk) and t002's missing.
        if ($env:TS_SETTLE) {
            $dir = Join-Path $env:TS_OUT 'term-cards\check'; New-Item -ItemType Directory -Force $dir | Out-Null
            $first = if ($env:TS_SETTLE -eq 'review') { 'needs_review' } else { 'ready' }
            ConvertTo-Json -Depth 8 @{ plan_revision = 3; terms = @(@{ row = 1; job = 't001'; status = $first }, @{ row = 2; job = 't002'; status = 'missing' }, @{ row = 3; job = 't003'; status = 'missing' }) } | Set-Content -LiteralPath (Join-Path $dir 'readiness.json') -Encoding utf8
        }
        Write-Output (ConvertTo-Json -Compress @{ ok = $true; rows = 2 }); exit 0
    }
    '^term-cards -Step review-plan' {
        $answers = Join-Path $env:TS_OUT 'term-cards\review\answers'
        $stem = if ($env:TS_REBIND) { 't001-r001-cccc0000-dddd0000' } else { 't001-r001-aaaa0000-bbbb0000' }
        Write-Output (ConvertTo-Json -Compress -Depth 8 @{ ok = $true; jobs = @(
                    @{ id = 't001-r001-review'; row = 1; delete_after_collect = $true; files = @('instr.md', 'request.md'); prompt = 'p'; answer = (Join-Path $answers "$stem.json"); writes = $answers },
                    @{ id = 'r2'; delete_after_collect = $false }); counts = @{ rows = 2; needs_review = 1 } })
        exit 0
    }
    '^ledger done' {
        if ($env:TS_DONE_REFUSE -and $flat -match 'termcards/book@1') { Write-Output 'zt-digest ledger: the book is not ready'; exit 1 }
        if ($env:TS_STALE -and $flat -match 'termcards/t001@1 -Token 1 ') { Write-Output 'zt-digest ledger: stale_claim: termcards/t001@1 has no current unexpired claim for token 1'; exit 1 }
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
$env:TS_NEXT_COUNT = Join-Path $scratch 'next-page.txt'
$env:TS_STALE_NEXT = Join-Path $scratch 'stale-next.txt'

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

# 1b. v10 (atkinson2015lectures, 2026-11-01): the start claims EVERY ready job, page after page of at most 64 units,
# under a lease long enough for the whole queue; a non-job unit a page hands out is released at once.
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue
Set-Content -LiteralPath $calls -Value '' -Encoding utf8
$env:TS_TWO_PAGES = '1'
$all = Step (@('-Step', 'start') + $common)
$env:TS_TWO_PAGES = ''
Assert ($all.json.ok -eq $true -and $all.json.jobs -eq 3) "start v10: two pages of claims list all three jobs ($($all.text))"
$allCalls = @(Get-Content -LiteralPath $calls)
$nexts = @($allCalls | Where-Object { $_ -match '^ledger next .*-Layer termcards' })
Assert ($nexts.Count -eq 3 -and @($nexts | Where-Object { $_ -match '-Take 64' -and $_ -match '-Lease 7200' }).Count -eq 3) "start v10: it claims until a page is empty, 64 at a time, under the ledger's longest lease ($($nexts -join ' / '))"
Assert (@($allCalls | Where-Object { $_ -match '^ledger reclaim .*-Unit termcards/book@1 -Release' }).Count -eq 1) 'start v10: the book unit a page hands out is released at once'
Assert (@($allCalls | Where-Object { $_ -match '^term-cards -Step jobs .*t001:1,t002:2,t003:3' }).Count -eq 1) 'start v10: one job list names every claimed job with its token'
# A math job writes, self-checks, reviews and fixes its own draft, and writes its verdict: one DeepSeek job, no
# separate review round.
$jobs = @(Get-Content -Raw (Join-Path $out 'term-cards\drive\jobs.json') | ConvertFrom-Json)
$math = $jobs | Where-Object { $_.id -eq 't002' }
$concept = $jobs | Where-Object { $_.id -eq 't001' }
$verdict = Join-Path $out 'term-cards\review\answers\t002-r002-self.json'
Assert (@($math.files -match 'dsh-term-review-instructions\.zh\.md$').Count -eq 1 -and $math.files[-1] -match 'jobs\\t002\.md$') "start v10: a math job reads the review instructions too ($($math.files -join ', '))"
Assert ($math.prompt -match '审查' -and $math.prompt -match '修正|改' -and $math.prompt.Contains($verdict)) "start v10: a math job's prompt asks it to review and fix its draft and write its verdict to $verdict ($($math.prompt))"
$conceptVerdict = Join-Path $out 'term-cards\review\answers\t001-r001-self.json'
Assert ($concept.prompt.StartsWith('p') -and $concept.prompt.Contains($conceptVerdict) -and @($concept.files -match 'dsh-term-review-instructions\.zh\.md$').Count -eq 1) "start v10: a concept job keeps its writer prompt and, for a term it promotes to math, reviews it and writes its verdict ($($concept.prompt))"
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue

# 1c. v10 (atkinson2015lectures, 2026-11-01): a rebuilt zt-digest registered plan revision 3 and every job was open
# again, although 62 jobs had written drafts whose rows were published. The start checks the drafts on disk and
# completes a claimed job whose rows are all ready at once, so it is never written again.
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue
Set-Content -LiteralPath $calls -Value '' -Encoding utf8
$env:TS_TWO_PAGES = '1'; $env:TS_SETTLE = '1'
$settle = Step (@('-Step', 'start') + $common)
$env:TS_TWO_PAGES = ''; $env:TS_SETTLE = ''
$settleCalls = @(Get-Content -LiteralPath $calls)
Assert ($settle.json.ok -eq $true -and $settle.json.jobs -eq 2 -and $settle.json.settled -eq 1) "start v10: a claimed job whose rows are ready is settled, the other two are listed ($($settle.text))"
Assert (@($settleCalls | Where-Object { $_ -match '^ledger done .*-Unit termcards/t001@1 -Token 1 ' }).Count -eq 1) 'start v10: the settled job completes through ledger done with its claim token'
Assert (@($settleCalls | Where-Object { $_ -match '^term-cards -Step jobs ' -and $_ -match 't002:2,t003:3' -and $_ -notmatch 't001:' }).Count -eq 1) 'start v10: the job list names only the open jobs'

# 1d. v10 (atkinson2015lectures, 2026-11-02): 104 reopened jobs had passing drafts that lacked only the review of a
# promoted term. The start lists such a job as one review-and-fix job (the review plan's request), never as a writer.
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue
Set-Content -LiteralPath $calls -Value '' -Encoding utf8
$env:TS_TWO_PAGES = '1'; $env:TS_SETTLE = 'review'
$rev = Step (@('-Step', 'start') + $common)
$env:TS_TWO_PAGES = ''; $env:TS_SETTLE = ''
$revCalls = @(Get-Content -LiteralPath $calls)
$revJobs = @(Get-Content -Raw (Join-Path $out 'term-cards\drive\jobs.json') | ConvertFrom-Json)
Assert ($rev.json.ok -eq $true -and $rev.json.jobs -eq 3 -and $rev.json.reviews -eq 1 -and -not $rev.json.settled) "start v10: a job lacking only its review is listed as one review job beside two writers ($($rev.text))"
Assert (@($revCalls | Where-Object { $_ -match '^term-cards -Step jobs ' -and $_ -match 't002:2,t003:3' -and $_ -notmatch 't001:' }).Count -eq 1) 'start v10: the review-only job gets no writer job'
# v11: one chunk file per manager holds at most -Chunk entries of the job list, in order.
Remove-Item -LiteralPath (Join-Path $out 'term-cards\check\readiness.json') -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue
$chunked = Step (@('-Step', 'start', '-Chunk', '2') + $common)
$c1 = @(Get-Content -Raw (Join-Path $out 'term-cards\drive\jobs-001.json') | ConvertFrom-Json)
Assert ($chunked.json.chunks -eq 1 -and $c1.Count -eq 2 -and -not (Test-Path -LiteralPath (Join-Path $out 'term-cards\drive\jobs-002.json'))) "start v11: two jobs make one chunk of two, and an old chunk file is removed ($($chunked.text))"
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue
$env:TS_TWO_PAGES = '1'
$chunked3 = Step (@('-Step', 'start', '-Chunk', '2') + $common)
$env:TS_TWO_PAGES = ''
$c1 = @(Get-Content -Raw (Join-Path $out 'term-cards\drive\jobs-001.json') | ConvertFrom-Json)
$c2 = @(Get-Content -Raw (Join-Path $out 'term-cards\drive\jobs-002.json') | ConvertFrom-Json)
Assert ($chunked3.json.chunks -eq 2 -and ($c1.id -join ',') -eq 't001,t002' -and ($c2.id -join ',') -eq 't003') "start v11: three jobs make chunks of two and one, in job-list order ($($chunked3.text))"
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue
$rj1 = $revJobs | Where-Object { $_.id -eq 't001-r001-review' }
Assert ($rj1 -and $rj1.prompt -match '修正' -and @($rj1.files -match 'dsh-term-card-instructions\.zh\.md$').Count -eq 1 -and $rj1.writes -eq (Join-Path $out 'term-cards')) "start v10: the review job reviews and fixes the draft with the writer's instructions ($($rj1 | ConvertTo-Json -Compress))"
Remove-Item -LiteralPath (Join-Path $out 'term-cards\check\readiness.json') -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue
$null = Step (@('-Step', 'start') + $common)
Remove-Item -LiteralPath $env:TS_NEXT_COUNT -ErrorAction SilentlyContinue

# 2. review lists its jobs, and its counts stay out of the line. Since 2026-10-26 one DeepSeek job reviews AND fixes a
# draft: it reads the writer's instructions and job file (the draft path and its SELF-CHECK), may write under the
# Term-card folder, and there is no separate fix step.
$review = Step (@('-Step', 'review') + $common)
Assert-Line $review 'review'
Assert ($review.json.ok -eq $true -and $review.json.reviews -eq 1 -and -not $review.json.PSObject.Properties['counts']) "review: one review job, no nested counts in the line ($($review.text))"
$reviews = Get-Content -Raw (Join-Path $out 'term-cards\drive\reviews.json') | ConvertFrom-Json
$rj = @($reviews)[0]
Assert ($rj.files[0] -eq 'instr.md' -and @($rj.files -match 'dsh-term-card-instructions\.zh\.md$').Count -eq 1 -and @($rj.files -match 'term-cards\\jobs\\t001\.md$').Count -eq 1 -and $rj.files[-1] -eq 'request.md') "review: the job reads the review instructions, the writer's instructions and job file, then its request ($($rj.files -join ', '))"
Assert ($rj.writes -eq (Join-Path $out 'term-cards') -and $rj.prompt -match '修正') 'review: the job may write the draft and its answer, and its prompt asks it to fix'
$noFix = Step (@('-Step', 'fix') + $common)
Assert (-not ($noFix.json -and $noFix.json.ok -eq $true)) 'review: no separate fix step remains'

# 2b. finish binds the combined job's verdict to the draft it left: an ok answer written in this round (v10: after the
# start listed the jobs, drive\jobs.json) is copied to the current draft's answer path, also when the job touched the
# draft again after writing it (live 2026-10-26, acemoglu2025nobel row 15); an answer from before this round is never
# carried over.
$answersDir = Join-Path $out 'term-cards\review\answers'
$outDir = Join-Path $out 'term-cards\out'
$reviewsFile = Join-Path $out 'term-cards\drive\jobs.json'
New-Item -ItemType Directory -Force $answersDir, $outDir | Out-Null
Set-Content -LiteralPath (Join-Path $outDir 't001.json') -Value '{}' -Encoding utf8
$oldAnswer = Join-Path $answersDir 't001-r001-aaaa0000-bbbb0000.json'
Set-Content -LiteralPath $oldAnswer -Value '{"ok":true,"fixed":2}' -Encoding utf8
(Get-Item $reviewsFile).LastWriteTime = (Get-Date).AddMinutes(-10)
(Get-Item $oldAnswer).LastWriteTime = (Get-Date).AddMinutes(-5)
(Get-Item (Join-Path $outDir 't001.json')).LastWriteTime = (Get-Date).AddMinutes(-2)
$env:TS_REBIND = '1'
$bound = Step (@('-Step', 'finish') + $common)
$newAnswer = Join-Path $answersDir 't001-r001-cccc0000-dddd0000.json'
Assert ((Test-Path -LiteralPath $newAnswer) -and ((Get-Content -Raw $newAnswer | ConvertFrom-Json).ok -eq $true)) "finish: the combined job's ok verdict of this round is bound to the draft it left, even when it touched the draft afterwards ($($bound.text))"
Remove-Item -LiteralPath $newAnswer
(Get-Item $oldAnswer).LastWriteTime = (Get-Date).AddMinutes(-20)
$null = Step (@('-Step', 'finish') + $common)
$env:TS_REBIND = ''
Assert (-not (Test-Path -LiteralPath $newAnswer)) 'finish: a verdict from before this review round is never carried over'

# 3. finish: each claimed job completes from the readiness result and the book publishes; the line answers counts.
$finish = Step (@('-Step', 'finish') + $common)
Assert-Line $finish 'finish'
Assert ($finish.json.ok -eq $true -and $finish.json.done -eq 2 -and -not $finish.json.failed -and $finish.json.book -eq 'done') "finish: both jobs done and the book published ($($finish.text))"
Assert ($finish.json.complete -eq $true -and $finish.json.rows -eq 2 -and $finish.json.committed -eq 2) 'finish: the publication answers flat counts, never a nested object'
$finishDetail = Get-Content -LiteralPath $finish.json.detail -Raw | ConvertFrom-Json
Assert (@($finishDetail.detail.done).Count -eq 2 -and $finishDetail.detail.publication.complete -eq $true) 'finish: the detail file keeps the done job ids and the publication'

# 3b. v10: the queue claims every job at once, so a job may outlast its claim's lease (at most 7200 s). The finish
# takes such a claim back (reclaim, then claim it again) and completes the job under the new token.
Set-Content -LiteralPath $calls -Value '' -Encoding utf8
Remove-Item -LiteralPath $env:TS_STALE_NEXT -ErrorAction SilentlyContinue
$env:TS_STALE = '1'
$late = Step (@('-Step', 'finish') + $common)
$env:TS_STALE = ''
$lateCalls = @(Get-Content -LiteralPath $calls)
Assert ($late.json.ok -eq $true -and $late.json.done -eq 2 -and -not $late.json.failed) "finish v10: a job whose lease ran out is still completed ($($late.text))"
Assert (@($lateCalls | Where-Object { $_ -match '^ledger reclaim .*-Unit termcards/t001@1$' }).Count -eq 1 -and @($lateCalls | Where-Object { $_ -match '^ledger done .*-Unit termcards/t001@1 -Token 8 ' }).Count -eq 1) "finish v10: the expired claim is taken back and the job completes under its new token ($($lateCalls -join ' / '))"

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

# 6. no ready unit because the pass is already complete: the book is linked again, never skipped.
# Live 2026-10-27: mode cards rewrote 421 Literature notes after the pass; a skip left them without Term links.
Set-Content -LiteralPath $calls -Value '' -Encoding utf8
$env:TS_NO_LINK_UNIT = '1'
$relinked = Step (@('-Step', 'link') + $common)
$env:TS_NO_LINK_UNIT = ''
Assert-Line $relinked 'link relinked'
Assert ($relinked.json.ok -eq $true -and $relinked.json.status -eq 'relinked' -and $relinked.json.cards_changed -eq 1) "link: an already complete pass links the Cards changed since ($($relinked.text))"
$relinkCalls = @(Get-Content -LiteralPath $calls | Where-Object { $_ -match '^term-link -Step apply' })
Assert ($relinkCalls.Count -eq 1 -and $relinkCalls[0] -notmatch '-Token') 'link: the relink applies without a claim, so the done unit stays done'

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



