<#
.SYNOPSIS
  The Term stages' deterministic command chains, one step per call (#172, #180). A DeepSeek job runs one
  step and replies with its last line, so no Claude relay runs a single command.

  pwsh -NoProfile -File term-steps.ps1 -Step start|review|finish|link -Out <staging> [-Occurrences first|all]

  start   plan the Term cards, claim every ready job unit (page after page of at most 64, under the ledger's
          longest lease), complete at once a claimed job whose rows are already ready, list the others; a
          math job also reviews and fixes its own draft and writes its verdict (v10) -> drive\jobs.json
  review  check the drafts and plan the review-and-fix jobs (one DeepSeek job reviews a draft, fixes it
          and runs its self-check); the v10 module no longer calls it     -> drive\reviews.json
  finish  complete each claimed job from the readiness result (fail the rest; a claim whose lease ran out
          is taken back first), then publish:
          with the book unit claimed it completes from the report, else every ready row is published (#182).
          -Retry (v19): a job that is not ready is released for one more writer round instead of failed; when
          any is, nothing is published and the line answers `retry`
  link    plan the Term links, apply under the claim (or, when the unit is blocked, the published
          prefix without a claim; when the unit is already done, the whole book again without a
          claim, status relinked), then plan again: the second plan must change no Card

  Each step prints ONE compact JSON line (#180): `ok` first, then the counts and file paths the module
  reads, at most 400 characters; the full data (the units, the publication, the counts) goes to
  <staging>\term-cards\drive-lines\<step>.json, named in the line as `detail`. Files stay under
  <staging>\term-cards\drive.
#>
param(
    [Parameter(Mandatory)][ValidateSet('start', 'review', 'finish', 'link')][string]$Step,
    [Parameter(Mandatory)][string]$Out,
    [ValidateSet('first', 'all')][string]$Occurrences = 'first',
    [int]$PerJob = 0,
    [int]$MathPerJob = 0,
    [ValidateRange(1, 8)][int]$Chunk = 8,
    [switch]$Retry,
    [string]$Tool = 'zt-digest'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'step-line.ps1')
$Out = $Out.TrimEnd('\', '/')
$Work = Join-Path $Out 'term-cards'
$Drive = Join-Path $Work 'drive'
New-Item -ItemType Directory -Force $Drive | Out-Null
$LineName = $Step

# One command: its last JSON line (or $null) and its exit code.
function Run([string[]]$CommandArgs) {
    $text = & $Tool @CommandArgs 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $line = @($text | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($line) { try { $json = $line | ConvertFrom-Json -Depth 64 } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; tail = (@($text) | Select-Object -Last 3) -join ' | ' }
}
function Save($Name, $Value) { ConvertTo-Json -InputObject $Value -Depth 64 | Set-Content -LiteralPath (Join-Path $Drive $Name) -Encoding utf8 }
function Load($Name) { $p = Join-Path $Drive $Name; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64 } }
# The step's answer: its compact line and the detail file beside it (#180).
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
# Validate a `ledger done -UnitsFile` batch reply strictly by reply kind, no fatal stop_reason,
# and exact (unit, token) coverage.
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

# 2026-10-26: one DeepSeek job reviews AND fixes a draft. Besides its request it reads the writer's instructions and the
# writer's job file (the draft path and its SELF-CHECK), and it may write under the Term-card folder: the draft it fixes
# and its answer. There is no separate fix step.
function Review-Job($j) {
    $writer = "$($j.id)" -replace '-r\d+-review$', ''
    $files = @($j.files)
    $j | Add-Member -Force files (@($files[0], (Join-Path $Work 'dsh-term-card-instructions.zh.md'), (Join-Path $Work "jobs\$writer.md")) + @($files | Select-Object -Skip 1))
    $j | Add-Member -Force prompt '按审查与修正说明审查这张数学术语卡草稿：先审查，再把错误直接改在草稿文件里并运行自查，然后只写任务文件指定的答案文件，最后只回复一行 JSON。'
    $j | Add-Member -Force writes $Work
    $j
}

switch ($Step) {
    'start' {
        # A start begins the branch: release any Term-card claim a stopped run held in one command.
        $rel = Run @('ledger', 'reclaim', '-Out', $Out, '-Layer', 'termcards', '-All', '-Release')
        if ($rel.code -ne 0) { Fail 'release' $rel.tail; break }
        if ($rel.json -and (@($rel.json.blocked).Count -or @($rel.json.skipped).Count)) {
            $first = @(@($rel.json.blocked) + @($rel.json.skipped))[0]
            Fail 'release' "$($first.unit): $($first.reason)"; break
        }
        $planArgs = @('term-cards', '-Step', 'plan', '-Staging', $Out)
        if ($PerJob -gt 0) { $planArgs += @('-PerJob', "$PerJob") }
        if ($MathPerJob -gt 0) { $planArgs += @('-MathPerJob', "$MathPerJob") }
        $plan = Run $planArgs
        $pStop = Planning-Stop $plan
        if ($pStop) { Fail-PlanningStop 'plan' $pStop @{ refusal = $pStop.refusal }; break }
        if (-not $plan.json -or $plan.json.ok -ne $true) { Fail 'plan' $plan.tail; break }
        # Claim every ready Term-card job unit in one transaction without touching termcards/book@*.
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termcards', '-Prefix', 'termcards/t', '-All', '-Owner', 'term cards', '-Lease', '7200')
        if ($claim.code -ne 0 -or -not $claim.json) { Fail 'claim' $claim.tail; break }
        $units = @($claim.json.units | Where-Object { $_ -and "$($_.id)".StartsWith('termcards/t') })
        # Settle already-ready drafts in one batch command; review-only jobs go to review-plan.
        $settled = @(); $reviewOnly = @(); $reviewUnits = @()
        if ($units.Count) {
            Run @('term-cards', '-Step', 'check', '-Staging', $Out) | Out-Null
            $readyPath = Join-Path $Work 'check\readiness.json'
            if (Test-Path -LiteralPath $readyPath) {
                $terms = @((Get-Content -LiteralPath $readyPath -Raw -Encoding utf8 | ConvertFrom-Json -Depth 64).terms)
                $candidates = @(); $open = @()
                foreach ($u in $units) {
                    $id = "$($u.inputs.job.id)"
                    $rows = @($terms | Where-Object { "$($_.job)" -eq $id })
                    if ($rows.Count -and -not @($rows | Where-Object { $_.status -ne 'ready' }).Count) {
                        $candidates += $u
                        continue
                    }
                    if ($rows.Count -and -not @($rows | Where-Object { $_.status -ne 'ready' -and $_.status -ne 'needs_review' }).Count) {
                        $reviewOnly += $id; $reviewUnits += $u; continue
                    }
                    $open += $u
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
                        # Reconcile against the ledger before dispatching writers so committed prefixes are not rewritten.
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
        Save 'units.json' (@($units) + @($reviewUnits))
        if (-not $units.Count -and -not $reviewUnits.Count) { Save 'jobs.json' @(); Reply @{ ok = $true; jobs = 0; settled = $settled.Count; plan_revision = [int]$plan.json.revision } @{ units = @(); settled = $settled; plan = $plan.json }; break }
        $writerJobs = @()
        if ($units.Count) {
            $tokens = ($units | ForEach-Object { "$($_.inputs.job.id):$($_.token)" }) -join ','
            $jobs = Run @('term-cards', '-Step', 'jobs', '-Staging', $Out, '-Tokens', $tokens)
            if (-not $jobs.json -or $jobs.json.ok -ne $true) { Fail 'jobs' $jobs.tail; break }
            $writerJobs = @($jobs.json.jobs)
        }
        # v10 (the user's design, 2026-11-01): one DeepSeek job writes a math term, self-checks it, reviews and fixes
        # its own draft against the review instructions, and writes its verdict beside the review answers; the finish
        # binds a verdict written after this job list to the draft the job left. A concept job may promote a term to
        # math (2026-11-02: 155 promoted rows had no review), so it carries the same review for such a term.
        $answersDir = Join-Path $Work 'review\answers'
        New-Item -ItemType Directory -Force $answersDir | Out-Null
        # The run keeps the exact review text its jobs were asked against (the compiler's review-plan copies the same file).
        $reviewText = Join-Path $PSScriptRoot 'dsh-term-review-instructions.zh.md'
        if (Test-Path -LiteralPath $reviewText) { Copy-Item -LiteralPath $reviewText -Destination (Join-Path $Work 'dsh-term-review-instructions.zh.md') -Force }
        # A concept job may promote a term to math: each message (a job of a writer conversation, 2026-11-08) names the
        # verdict files of its own rows. The review instructions travel once, with the conversation's first message.
        $promote = {
            param($j)
            $verdicts = @(@($j.rows) | ForEach-Object { Join-Path $answersDir ('{0}-r{1:D3}-self.json' -f $j.id, [int]$_) })
            $j | Add-Member -Force prompt ("$($j.prompt)" + ' 如果你把其中某个术语提升为数学概念：写完并自查通过后，再按审查与修正说明像独立审查者那样核对并修正这个术语，再运行自查直到通过，然后把审查结论写入它对应的文件（按术语表行号：' + ($verdicts -join '、') + '；通过：{"ok":true}；不通过：{"ok":false,"faults":[{"where":"…","what":"…","fix":"…"}]}）。没有提升的术语不写结论文件。')
            $j | Add-Member -Force verdicts $verdicts
        }
        foreach ($j in @($writerJobs | Where-Object { $_.kind -ne 'math' })) {
            $files = @($j.files)
            $j | Add-Member -Force files (@($files[0], (Join-Path $Work 'dsh-term-review-instructions.zh.md')) + @($files | Select-Object -Skip 1))
            & $promote $j
            foreach ($m in @($j.then | Where-Object { $_ })) { & $promote $m }
        }
        foreach ($j in @($writerJobs | Where-Object { $_.kind -eq 'math' })) {
            $verdicts = @(@($j.rows) | ForEach-Object { Join-Path $answersDir ('{0}-r{1:D3}-self.json' -f $j.id, [int]$_) })
            $files = @($j.files)
            $j | Add-Member -Force files (@($files[0], (Join-Path $Work 'dsh-term-review-instructions.zh.md')) + @($files | Select-Object -Skip 1))
            $j | Add-Member -Force prompt ('按共享说明和任务文件写好本组术语的 JSON 草稿，运行任务文件里的自查直到通过。然后按审查与修正说明，像一位独立审查者那样逐项核对你自己的草稿（对照原文小节），把错误直接改在草稿文件里，再运行自查直到通过。最后把审查结论写入 ' + ($verdicts -join '、') + '（每个数学术语一个文件；通过：{"ok":true}；不通过：{"ok":false,"faults":[{"where":"…","what":"…","fix":"…"}]}）。只写任务文件指定的输出文件和这些结论文件，最后只回复一行 JSON。')
            $j | Add-Member -Force verdicts $verdicts
        }
        $reviewJobs = @()
        if ($reviewOnly.Count) {
            $r = Run @('term-cards', '-Step', 'review-plan', '-Staging', $Out)
            if (-not $r.json -or $r.json.ok -ne $true) { Fail 'review-plan' $r.tail; break }
            $reviewJobs = @($r.json.jobs | Where-Object { $_.delete_after_collect -eq $true -and $reviewOnly -contains ("$($_.id)" -replace '-r\d+-review$', '') } | ForEach-Object { Review-Job $_ })
        }
        $all = @($writerJobs) + @($reviewJobs)
        Save 'jobs.json' $all
        # v11 (atkinson2015lectures, 2026-11-02): two managers told to run "entries 232 through 239 (0-based)" ran the
        # job ids t232..t239 instead, and 16 jobs never ran. Each manager now gets its own chunk file of at most
        # -Chunk jobs (the DSMP manager size) and runs every entry in it.
        Get-ChildItem -LiteralPath $Drive -Filter 'jobs-*.json' -ErrorAction SilentlyContinue | Remove-Item -Force
        $chunks = [int][Math]::Ceiling($all.Count / $Chunk)
        for ($k = 0; $k -lt $chunks; $k++) {
            Save ('jobs-{0:D3}.json' -f ($k + 1)) @($all | Select-Object -Skip ($k * $Chunk) -First $Chunk)
        }
        $messages = $all.Count + @($all | ForEach-Object { @($_.then | Where-Object { $_ }) }).Count
        Reply @{ ok = $true; jobs = $all.Count; messages = $messages; chunks = $chunks; reviews = $reviewJobs.Count; settled = $settled.Count; plan_revision = [int]$plan.json.revision; file = (Join-Path $Drive 'jobs.json') } @{ units = (@($units) + @($reviewUnits)); settled = $settled; review_only = $reviewOnly; plan = $plan.json; jobs = $all }
    }
    'review' {
        Run @('term-cards', '-Step', 'check', '-Staging', $Out) | Out-Null
        $r = Run @('term-cards', '-Step', 'review-plan', '-Staging', $Out)
        if (-not $r.json -or $r.json.ok -ne $true) { Fail 'review-plan' $r.tail; break }
        $jobs = @($r.json.jobs | Where-Object { $_.delete_after_collect -eq $true } | ForEach-Object { Review-Job $_ })
        Save 'reviews.json' $jobs
        Reply @{ ok = $true; reviews = $jobs.Count; file = (Join-Path $Drive 'reviews.json') } @{ reviews = $jobs; counts = $r.json.counts }
    }
    'finish' {
        Run @('term-cards', '-Step', 'check', '-Staging', $Out) | Out-Null
        # The combined job's verdict is bound to the draft it left: when its fix changed the draft, the review the
        # current draft needs has no answer yet, and the job's own `ok` answer of this round is that answer. After
        # the start no one but that job changes the draft, so an answer written after this round's job list
        # (drive\jobs.json, v10) binds, even when the job touched the draft again afterwards (live 2026-10-26,
        # acemoglu2025nobel row 15); an answer from before this round is never carried over.
        $round = Get-Item -LiteralPath (Join-Path $Drive 'jobs.json') -ErrorAction SilentlyContinue
        $rp = Run @('term-cards', '-Step', 'review-plan', '-Staging', $Out)
        $answersDir = Join-Path $Work 'review\answers'
        $bound = 0
        foreach ($j in @($rp.json.jobs | Where-Object { $_.delete_after_collect -eq $true -and $_.answer })) {
            if (Test-Path -LiteralPath $j.answer) { continue }
            $stem = [IO.Path]::GetFileNameWithoutExtension("$($j.answer)")
            $prefix = $stem -replace '^(.+-r\d{3}-).*$', '$1'
            $old = @(Get-ChildItem -LiteralPath $answersDir -Filter "$prefix*.json" -ErrorAction SilentlyContinue | Where-Object { $_.BaseName -ne $stem } | Sort-Object LastWriteTime -Descending) | Select-Object -First 1
            if (-not $old -or -not $round -or $old.LastWriteTime -lt $round.LastWriteTime) { continue }
            $verdict = $null
            try { $verdict = Get-Content -LiteralPath $old.FullName -Raw -Encoding utf8 | ConvertFrom-Json } catch { $verdict = $null }
            if ($verdict -and $verdict.ok -eq $true) { Copy-Item -LiteralPath $old.FullName -Destination $j.answer; $bound++ }
        }
        if ($bound) { Run @('term-cards', '-Step', 'check', '-Staging', $Out) | Out-Null }
        $done = @(); $failed = @(); $late = @(); $retried = @(); $retryCandidates = @(); $retryBlocked = @()
        $readiness = Join-Path $Work 'check\readiness.json'
        $heldUnits = @(Load 'units.json' | Where-Object { $_ -and $_.id })
        if ($heldUnits.Count) {
            $db = Run @('ledger', 'done', '-Out', $Out, '-UnitsFile', (Join-Path $Drive 'units.json'), '-Outputs', $readiness)
            $valid = Validate-DoneBatch $db $heldUnits
            if (-not $valid.ok) {
                # Missing, malformed, wrong-kind, or truncated batch reply: reconcile against ledger
                # before spending any attempt; never turn a batch-level error into content faults.
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
                    elseif ($Retry) { $retryCandidates += $u }
                    else {
                        $cause = if ($r.cause) { "$($r.cause)" } else { "$($db.tail)" }
                        $failed += $u.inputs.job.id; Fail-Unit $u "the job $($u.inputs.job.id) is not ready: $cause"
                    }
                }
            }
        }
        if ($retryCandidates.Count) {
            # Group by owner so voluntary owner-and-token-fenced surrender works inside the quiet
            # window while still respecting `unit_guard` and `jobs_running`. Never fail blocked/recent claims.
            $releasedIds = @{}
            $byOwner = @{}
            foreach ($u in $retryCandidates) {
                $ow = if ($u.owner) { "$($u.owner)" } else { 'term cards' }
                if (-not $byOwner.ContainsKey($ow)) { $byOwner[$ow] = @() }
                $byOwner[$ow] += $u
            }
            $gIdx = 0
            foreach ($ow in $byOwner.Keys) {
                $gIdx++
                $group = @($byOwner[$ow])
                $rFile = if ($byOwner.Count -eq 1) { 'retry-units.json' } else { "retry-units-$gIdx.json" }
                Save $rFile $group
                $relArgs = @('ledger', 'reclaim', '-Out', $Out, '-UnitsFile', (Join-Path $Drive $rFile), '-Release')
                if ($ow) { $relArgs += @('-Owner', $ow) }
                $rel = Run $relArgs
                if ($rel.code -ne 0 -or -not $rel.json) {
                    foreach ($u in $group) { $retryBlocked += @{ unit = "$($u.id)"; job = "$($u.inputs.job.id)"; reason = 'reclaim_error'; cause = "$($rel.tail)" } }
                    continue
                }
                foreach ($x in @($rel.json.reclaimed | Where-Object { $_ -and $_.unit })) { $releasedIds["$($x.unit)"] = $true }
                foreach ($b in @($rel.json.blocked | Where-Object { $_ -and $_.unit })) { $retryBlocked += @{ unit = "$($b.unit)"; reason = "$($b.reason)"; jobs = $b.jobs } }
                foreach ($s in @($rel.json.skipped | Where-Object { $_ -and $_.unit })) { $retryBlocked += @{ unit = "$($s.unit)"; reason = "$($s.reason)"; cause = "$($s.cause)" } }
            }
            foreach ($u in $retryCandidates) {
                if ($releasedIds.ContainsKey("$($u.id)")) { $retried += $u.inputs.job.id }
                elseif (-not @($retryBlocked | Where-Object { $_.unit -eq "$($u.id)" }).Count) {
                    $retryBlocked += @{ unit = "$($u.id)"; job = "$($u.inputs.job.id)"; reason = 'not_released' }
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
                $c = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termcards', '-IdsFile', (Join-Path $Drive 'late-ids.json'), '-All', '-Owner', 'term cards finish', '-Lease', '3600')
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
        # A retry round comes first: the publication waits for the jobs it holds (a late claim was completed above).
        if ($retryBlocked.Count -and -not $retried.Count) {
            Fail 'retry' "$($retryBlocked[0].unit): $($retryBlocked[0].reason)" @{ done = $done; failed = $failed; blocked = $retryBlocked }
            break
        }
        if ($retried.Count) {
            Reply @{ ok = $true; done = $done.Count; retry = $retried.Count; failed = $failed.Count; blocked = $retryBlocked.Count } @{ done = $done; retry = $retried; failed = $failed; blocked = $retryBlocked }
            break
        }
        function Book-State([string]$Layer) {
            $s = Run @('ledger', 'status', '-Out', $Out, '-Layer', $Layer)
            $held = @(@($s.json.units) | Where-Object { $_ -and "$($_.id)" -like "$Layer/book@*" }) | Select-Object -Last 1
            if ($held -and ($held.state -eq 'done' -or $held.state -eq 'noop')) { return 'done' }
            return 'incomplete'
        }
        $book = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termcards', '-Prefix', 'termcards/book@', '-Take', '1', '-Owner', 'term cards book', '-Wait', '60', '-Lease', '3600')
        $bu = @($book.json.units | Where-Object { $_ -and "$($_.id)".StartsWith('termcards/book@') }) | Select-Object -First 1
        if ($bu) {
            $a = Run @('term-cards', '-Step', 'apply', '-Staging', $Out, '-Token', "$($bu.token)", '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-PlanningStop 'apply' $aStop @{ done = $done; failed = $failed; refusal = $aStop.refusal }; break }
            if ($a.json -and $a.json.complete -eq $true) {
                $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $bu.id, '-Token', "$($bu.token)", '-Outputs', (Join-Path $Work 'report.json'))
                $bookState = if ($c.code -eq 0) { 'done' } else { 'failed' }
                if ($c.code -ne 0) { Fail-Unit $bu "the book completion was refused: $($c.tail)" }
            } else { $bookState = 'failed'; Fail-Unit $bu "the publication is not complete: $($a.tail)" }
        } else {
            # A failed job leaves the book unit blocked: its ready contiguous prefix is still published.
            $a = Run @('term-cards', '-Step', 'apply', '-Staging', $Out, '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-PlanningStop 'apply' $aStop @{ done = $done; failed = $failed; refusal = $aStop.refusal }; break }
            # A resume finds the book unit already done in an earlier invocation (live 2026-11-13, bergin2026exchange):
            # nothing is claimable, and the ledger's own state answers.
            $bookState = Book-State 'termcards'
        }
        $publication = @{ complete = ($a.json.complete -eq $true); rows = [int]$a.json.rows; committed = [int]$a.json.committed; ready_unpublished = [int]$a.json.ready_unpublished; stop = $a.json.stop }
        # Live run 2026-10-26 (staging5): an apply that prints no line is named, never read as zero rows.
        # An apply that answers {ok:false, code} (staging6: `plan_stale`) is named by its code and hint.
        $publishError = if (-not $a.json) { "$($a.tail)".Trim() } elseif ($a.json.ok -eq $false) { "$($a.json.code): $($a.json.hint)".Trim() } else { '' }
        if ($publishError.Length -gt 160) { $publishError = $publishError.Substring($publishError.Length - 160) }
        $publication.error = $publishError
        Reply @{ ok = $true; done = $done.Count; failed = $failed.Count; book = $bookState; complete = $publication.complete; rows = $publication.rows; committed = $publication.committed; ready_unpublished = $publication.ready_unpublished; publish_error = $publishError } @{ done = $done; failed = $failed; publication = $publication; apply = $a.tail }
    }
    'link' {
        $p = Run @('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $Occurrences)
        $pStop = Planning-Stop $p
        if ($pStop) { Fail-PlanningStop 'plan' $pStop @{ refusal = $pStop.refusal }; break }
        if (-not $p.json -or $p.json.ok -ne $true) { Fail 'plan' $p.tail; break }
        $claim = Run @('ledger', 'next', '-Out', $Out, '-Layer', 'termlink', '-Take', '1', '-Owner', 'term-link', '-Wait', '60', '-Lease', '3600')
        $u = @($claim.json.units | Where-Object { $_.id -like 'termlink/*' }) | Select-Object -First 1
        if ($u) {
            $a = Run @('term-link', '-Step', 'apply', '-Staging', $Out, '-Occurrences', $Occurrences, '-Token', "$($u.token)", '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-Unit $u "$($aStop.code): $($aStop.message)"; Fail-PlanningStop 'apply' $aStop @{ unit = $u.id; refusal = $aStop.refusal }; break }
            if ($a.json -and $a.json.complete -eq $true) {
                $c = Run @('ledger', 'done', '-Out', $Out, '-Unit', $u.id, '-Token', "$($u.token)", '-Outputs', (Join-Path $Out 'term-link\apply.json'))
                $status = if ($c.code -eq 0) { 'done' } else { 'failed' }
                if ($c.code -ne 0) { Fail-Unit $u "the Term-link completion was refused: $($c.tail)" }
            } else { $status = 'failed'; Fail-Unit $u "the Term-link pass stopped: $($a.tail)" }
        } else {
            $gate = Run @('ledger', 'gate', '-Out', $Out, '-Layer', 'termlink', '-Brief')
            if ($gate.json -and $gate.json.counts -and [int]$gate.json.counts.claimed -gt 0) {
                Fail 'link' 'termlink unit is actively claimed by another worker; refusing unclaimed apply' @{ gate = $gate.json }
                break
            }
            # A complete pass still links the Cards a later writer changed (mode cards); its unit stays done.
            # A unit blocked by an incomplete Term-card book links the published prefix; the unit stays open.
            $complete = $gate.json -and $gate.json.complete -eq $true
            $a = Run @('term-link', '-Step', 'apply', '-Staging', $Out, '-Occurrences', $Occurrences, '-Apply')
            $aStop = Planning-Stop $a
            if ($aStop) { Fail-PlanningStop 'apply' $aStop @{ refusal = $aStop.refusal }; break }
            $status = if (-not ($a.json -and $a.json.complete -eq $true)) { 'failed' } elseif ($complete) { 'relinked' } else { 'partial' }
        }
        $second = Run @('term-link', '-Step', 'plan', '-Staging', $Out, '-Occurrences', $Occurrences)
        $sStop = Planning-Stop $second
        if ($sStop) { Fail-PlanningStop 'plan' $sStop @{ refusal = $sStop.refusal }; break }
        Reply @{
            ok = $true; status = $status; unit = "$($u.id)"
            cards_changed = [int]$a.json.counts.cards_changed; links_added = [int]$a.json.counts.links_added
            second_cards_changed = [int]$second.json.cards_changed; second_cards_refused = [int]$second.json.cards_refused
        } @{ counts = $a.json.counts; second = @{ cards_changed = $second.json.cards_changed; cards_refused = $second.json.cards_refused }; plan = $p.json }
    }
}

