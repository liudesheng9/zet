#Requires -Version 7.5
<#
.SYNOPSIS
  End-to-end regression suite for P0/P1 batch acceleration and Astra audit invariants:
  1. Real 17-group scratch-archive drain (P0-1 group-start/finish batch, pre-claim budget stop,
     >DRAIN_CHUNK=16 multi-chunk cards drain, canonical alias refusal, voluntary owner+token
     surrender inside quiet window with jobs_running protection).
  2. Step-script boundary probes (step-line 320-char lock fit, cards remaining continuation,
     native Event apply lock stop before report, scope_refused preservation, transport stop
     not_run failure, untyped exit 2 classification, Term/Cited done_batch reconciliation and
     after_commit handling, Term -Retry voluntary surrender & blocked preservation).
#>
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scripts = Split-Path -Parent $PSScriptRoot
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "batch-accel-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'

$pass = 0; $fail = 0
function Check($cond, [string]$msg) {
    if ($cond) { $script:pass++; Write-Output "ok: $msg" }
    else { $script:fail++; Write-Output "FAIL: $msg" }
}
function Save-Json($path, $value) {
    [IO.Directory]::CreateDirectory((Split-Path -Parent $path)) | Out-Null
    [IO.File]::WriteAllText($path, (ConvertTo-Json -InputObject $value -Depth 30), $utf8)
}
function Run-Digest([string[]]$Arguments) {
    $lines = @(& $Digest @Arguments 2>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
    $jsonLine = @($lines | Where-Object { $_.Trim().StartsWith('{') }) | Select-Object -Last 1
    $json = $null
    if ($jsonLine) { try { $json = $jsonLine | ConvertFrom-Json -Depth 64 -AsHashtable } catch { $json = $null } }
    [pscustomobject]@{ code = $code; json = $json; text = ($lines -join ' | ') }
}

try {
    # --- Part 1: Real 17-group scratch-archive drain & ledger invariants ---
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $out = Join-Path $scratch 'prepared'
    $source = Join-Path $scratch 'book17.md'
    $options = Join-Path $scratch 'options.json'
    $sb = [Text.StringBuilder]::new()
    for ($i = 1; $i -le 17; $i++) {
        [void]$sb.AppendLine("# 第${i}节")
        [void]$sb.AppendLine("")
        [void]$sb.AppendLine("第${i}节研究企业的融资约束如何影响资本配置与长期投资选择。")
        [void]$sb.AppendLine("")
    }
    [void]$sb.AppendLine("# 参考")
    [void]$sb.AppendLine("未选择。")
    [IO.File]::WriteAllText($source, $sb.ToString(), $utf8)
    Save-Json $options @{ terms = $false; findings = $false; models = $false }

    $prep = Run-Digest @('prepare', '-Source', $source, '-Out', $out, '-CitationKey', 'Drain17', '-BibTeX', '@book{Drain17,title={Drain 17}}', '-StartHeading', '第1节', '-EndHeading', '参考', '-MinUnitChars', '1', '-MaxUnits', '1', '-MaxChars', '400', '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    if ($prep.code -ne 0) { Write-Output "PREP ERR: $($prep.text)" }
    Check ($prep.code -eq 0 -and $prep.json.groups.Count -eq 17) "prepared 17-group scratch book ($($prep.json.groups.Count) groups)"

    # Claim all 17 digest units in one transaction
    $claimed = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'digest', '-All', '-Owner', 'batch-test', '-Lease', '3600')
    Check ($claimed.code -eq 0 -and $claimed.json.units.Count -eq 17) "ledger next -All claimed all 17 digest units"
    $uFile = Join-Path $scratch 'digest-units.json'
    Save-Json $uFile @($claimed.json.units)

    # Start all 17 in one group-start -UnitsFile call (reusing TreeCache + single Zt)
    $started = Run-Digest @('group-start', '-Out', $out, '-UnitsFile', $uFile)
    Check ($started.code -eq 0 -and $started.json.ok -eq $true -and $started.json.started -eq 17 -and $started.json.failed -eq 0) "group-start -UnitsFile started all 17 groups in one process"

    # Write valid drafts for all 17 groups
    foreach ($r in $started.json.results) {
        $g = [int]$r.group
        $job = Get-Content -LiteralPath "$($r.job)" -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
        $anchor = "Drain17/$g"
        $draft = @{
            schema_version = 1
            unit = "$($r.unit)"
            token = [int]$r.token
            input_hash = "$($r.input_hash)"
            disposition = 'cards'
            calls = @(
                @{ tool = 'edit_card'; arguments = @{ at = $anchor; title = "第${g}节融资约束机制"; body = "第${g}节讨论融资约束如何改变企业投资选择，使资本配置偏离无摩擦水平。"; base_fingerprint = $job.inputs.anchor.fingerprint } },
                @{ tool = 'create_successor'; arguments = @{ at = $anchor; kind = 'side'; title = "第${g}节资本配置受融资约束限制"; body = "融资成本上升迫使企业缩减长期设备投入。投入不足会降低后续生产能力与产出水平。本节以此刻画约束机制。来源：[[$anchor]]" } }
            )
        }
        Save-Json "$($r.draft)" $draft
    }

    # Finish all 17 digest groups in ONE group-finish -UnitsFile call
    $finished = Run-Digest @('group-finish', '-Out', $out, '-UnitsFile', $uFile)
    Check ($finished.code -eq 0 -and $finished.json.done -eq 17 -and $finished.json.failed -eq 0 -and $finished.json.not_run -eq 0) "group-finish -UnitsFile completed all 17 groups in one process"

    # Canonical group alias refusal on finish-batch
    $alias = Run-Digest @('finish-batch', '-Out', $out, '-Units', 'cards/g1@1:1')
    Check ($alias.code -eq 2 -and $alias.text.Contains('canonical')) "finish-batch rejects noncanonical alias cards/g1@1 with exit 2"

    # Pre-claim budget expiry (budget=0) preserves stop_reason:"budget" and claims 0
    $env:ZT_FINISH_BATCH_BUDGET_SECONDS = '0'
    $zero = Run-Digest @('finish-batch', '-Out', $out, '-Layer', 'cards', '-All', '-Owner', 'cards-drain')
    Remove-Item Env:ZT_FINISH_BATCH_BUDGET_SECONDS
    Check ($zero.code -eq 0 -and $zero.json.claimed -eq 0 -and $zero.json.stop_reason -eq 'budget') "pre-claim budget expiry returns claimed=0 with stop_reason=budget"

    # Full 17-unit cards drain (> DRAIN_CHUNK=16) in one finish-batch -Layer cards -All call
    $drain = Run-Digest @('finish-batch', '-Out', $out, '-Layer', 'cards', '-All', '-Owner', 'cards-drain')
    Check ($drain.code -eq 0 -and $drain.json.claimed -eq 17 -and $drain.json.done -eq 17 -and $drain.json.not_run -eq 0 -and $drain.json.settled -eq $true -and $drain.json.counts.open -eq 0) "17-unit cards drain completed across >16 chunk with settled=true, open=0"

    # Verify every cards unit was admitted exactly once (attempts == 1) and zero claims remain
    $cStatus = Run-Digest @('ledger', 'status', '-Out', $out, '-Layer', 'cards')
    $badAttempts = @($cStatus.json.units | Where-Object { $_.attempts -ne 1 -or $_.state -ne 'done' })
    Check ($cStatus.json.units.Count -eq 17 -and $badAttempts.Count -eq 0) "all 17 cards units admitted once (attempts=1) with zero orphaned claims"

    # Voluntary owner-and-token-fenced surrender inside quiet window vs jobs_running protection
    $evClaim = Run-Digest @('ledger', 'next', '-Out', $out, '-Layer', 'events', '-Take', '2', '-Owner', 'retry-owner', '-Lease', '7200')
    Check ($evClaim.code -eq 0 -and $evClaim.json.units.Count -eq 2) "claimed 2 events units for quiet-window surrender test"
    $ev1 = $evClaim.json.units[0]; $ev2 = $evClaim.json.units[1]
    $jobReg = Join-Path $scratch 'job-reg.json'
    Save-Json $jobReg @{ schema_version = 1; id = 'ext-job-1'; kind = 'deepseek'; state = 'running' }
    $reg = Run-Digest @('ledger', 'job', '-Out', $out, '-Unit', "$($ev2.id)", '-Token', "$($ev2.token)", '-Register', $jobReg)
    Check ($reg.code -eq 0) "registered external running job on second events unit"
    $surrFile = Join-Path $scratch 'surrender.json'
    Save-Json $surrFile @(@{ id = $ev1.id; token = $ev1.token }, @{ id = $ev2.id; token = $ev2.token })
    # Without -Owner, both are skipped as claim_recent (with token preserved!)
    $noOwner = Run-Digest @('ledger', 'reclaim', '-Out', $out, '-UnitsFile', $surrFile, '-Release')
    Check ($noOwner.code -eq 0 -and $noOwner.json.reclaimed.Count -eq 0 -and $noOwner.json.skipped.Count -eq 2 -and $noOwner.json.skipped[0].reason -eq 'claim_recent' -and $noOwner.json.skipped[0].token -eq $ev1.token) "reclaim -Release without -Owner respects quiet window and records token in skipped"
    # With mismatched -Owner, ev1 is REJECTED with owner_mismatch (never released!)
    $wrongOwner = Run-Digest @('ledger', 'reclaim', '-Out', $out, '-UnitsFile', $surrFile, '-Release', '-Owner', 'other-worker')
    Check ($wrongOwner.code -eq 0 -and $wrongOwner.json.reclaimed.Count -eq 0 -and $wrongOwner.json.skipped.Count -eq 2 -and $wrongOwner.json.skipped[0].reason -eq 'owner_mismatch') "reclaim -Release with mismatched -Owner rejects with owner_mismatch"
    # With matching -Owner, ev1 surrenders immediately while ev2 is still blocked by jobs_running
    $withOwner = Run-Digest @('ledger', 'reclaim', '-Out', $out, '-UnitsFile', $surrFile, '-Release', '-Owner', 'retry-owner')
    Check ($withOwner.code -eq 0 -and $withOwner.json.reclaimed.Count -eq 1 -and $withOwner.json.reclaimed[0].unit -eq $ev1.id -and $withOwner.json.blocked.Count -eq 1 -and $withOwner.json.blocked[0].unit -eq $ev2.id -and $withOwner.json.blocked[0].reason -eq 'jobs_running') "owner+token-fenced surrender releases recent claim immediately while enforcing jobs_running"

    # --- Part 2: Step-script & step-line boundary probes ---
    . (Join-Path $scripts 'step-line.ps1')
    $fitDir = Join-Path $scratch 'fit-detail'
    New-Item -ItemType Directory -Force $fitDir | Out-Null
    $longMsg = 'L' * 320
    $detailPath = Join-Path $fitDir 'finish.json'
    Save-Json $detailPath @{ original = 'keep-full-detail'; message = $longMsg }
    $fitted = Step-FitLine ([ordered]@{ ok = $false; step = 'finish'; code = 'card_locked'; layer = 'events'; unit = 'events/g001@1'; message = $longMsg; error = "card_locked: $longMsg"; detail = $detailPath }) 'finish' $fitDir
    $fitObj = $fitted | ConvertFrom-Json
    $keptDetail = Get-Content -LiteralPath $detailPath -Raw -Encoding utf8 | ConvertFrom-Json
    Check ($fitted.Length -le 400 -and $fitObj.ok -eq $false -and $fitObj.code -eq 'card_locked' -and $keptDetail.original -eq 'keep-full-detail') "Step-FitLine preserves code=card_locked within 400 chars and does not overwrite full detail file"

    # Mock tool for layer-steps, model-steps, term-steps, cited-steps boundary tests
    $mockOut = Join-Path $scratch 'mock-out'
    New-Item -ItemType Directory -Force $mockOut | Out-Null
    $mockPs1 = Join-Path $scratch 'mock-zt-digest.ps1'
    $mockCmd = $mockPs1
    $mockLog = Join-Path $scratch 'mock-calls.log'

    @'
param([Parameter(ValueFromRemainingArguments)]$A)
$line = ($A -join ' ')
Add-Content -LiteralPath $env:MOCK_LOG -Value $line -Encoding utf8
$mode = $env:MOCK_MODE
if ($A[0] -eq 'ledger' -and $A[1] -eq 'gate' -and $mode -ne 'termlink_live_claim') {
    Write-Output '{"complete":true,"settled":true,"counts":{"open":0,"total":2}}'
    exit 0
}
if ($mode -eq 'cards_budget') {
    if ($A[0] -eq 'finish-batch' -and ($A -contains '-All')) {
        $n = if (Test-Path $env:MOCK_STATE) { [int](Get-Content $env:MOCK_STATE -Raw) } else { 0 }
        $n++; Set-Content -LiteralPath $env:MOCK_STATE -Value "$n"
        if ($n -eq 1) {
            $df = Join-Path (Split-Path $env:MOCK_STATE) 'detail-1.json'
            '{"remaining":[{"id":"cards/g002@1","token":2}]}' | Set-Content -LiteralPath $df -Encoding utf8
            Write-Output (@{ kind = 'finish_batch'; claimed = 2; done = 1; not_run = 1; stop_reason = 'budget'; file = $df; results = @(@{ unit = 'cards/g001@1'; token = 1; status = 'done' }, @{ unit = 'cards/g002@1'; token = 2; status = 'not_run' }) } | ConvertTo-Json -Compress -Depth 8)
            exit 0
        }
        Write-Output '{"kind":"finish_batch","claimed":0,"done":0,"not_run":0,"results":[]}'
        exit 0
    }
    if ($A[0] -eq 'finish-batch' -and ($A -contains '-UnitsFile')) {
        Write-Output '{"kind":"finish_batch","done":1,"not_run":0,"results":[{"unit":"cards/g002@1","token":2,"status":"done"}]}'
        exit 0
    }
}
if ($mode -eq 'event_apply_lock') {
    if ($A[0] -eq 'event' -and ($A -contains 'apply')) {
        Write-Output '{"ok":false,"code":"card_locked","error":"card_locked","message":"the Literature tree Book is locked by the owner","tree":"Book","lock_id":7}'
        exit 2
    }
    Write-Output '{"error":"should_not_be_called"}'
    exit 1
}
if ($mode -eq 'scope_refused_finish') {
    if ($A[0] -eq 'finish-batch') {
        Write-Output '{"kind":"finish_batch","code":"scope_refused","error":"scope_refused","stop_reason":"scope_refused","message":"outside Literature scope","results":[{"unit":"models/g001-m0000@1","token":1,"status":"failed","code":"scope_refused"}]}'
        exit 2
    }
}
if ($mode -eq 'untyped_exit2') {
    if ($A[0] -eq 'finish-batch') {
        Write-Output 'zt-digest finish-batch: unknown parameter -Bogus'
        exit 2
    }
}
if ($mode -eq 'transport_stop') {
    if ($A[0] -eq 'finish-batch') {
        Write-Output '{"kind":"finish_batch","done":1,"not_run":1,"stop_reason":"transport","results":[{"unit":"models/g001-m0000@1","token":1,"status":"done"},{"unit":"models/g002-m0000@1","token":2,"status":"not_run"}]}'
        exit 0
    }
}
if ($mode -eq 'term_truncated_done') {
    if ($A[0] -eq 'term-cards' -and ($A -contains 'check')) { Write-Output '{"ok":true}'; exit 0 }
    if ($A[0] -eq 'term-cards' -and ($A -contains 'review-plan')) { Write-Output '{"ok":true,"jobs":[]}'; exit 0 }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'done') { Write-Output '{"kind":"wrong_kind"}'; exit 1 }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'status') {
        Write-Output '{"units":[{"id":"termcards/t001@1","state":"done","token":1},{"id":"termcards/t002@1","state":"claimed","token":2}]}'
        exit 0
    }
}
if ($mode -eq 'term_retry_mixed') {
    if ($A[0] -eq 'term-cards' -and ($A -contains 'check')) { Write-Output '{"ok":true}'; exit 0 }
    if ($A[0] -eq 'term-cards' -and ($A -contains 'review-plan')) { Write-Output '{"ok":true,"jobs":[]}'; exit 0 }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'done') {
        Write-Output '{"kind":"ledger_done_batch","results":[{"unit":"termcards/t001@1","token":1,"status":"error","ledger_state":"done","committed":true},{"unit":"termcards/t002@1","token":2,"status":"error","cause":"not ready"},{"unit":"termcards/t003@1","token":3,"status":"error","cause":"not ready"}]}'
        exit 0
    }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'reclaim') {
        Write-Output '{"reclaimed":[{"unit":"termcards/t002@1"}],"blocked":[{"unit":"termcards/t003@1","reason":"jobs_running","jobs":["j1"]}],"skipped":[]}'
        exit 0
    }
}
if ($mode -eq 'term_readiness_changed') {
    if ($A[0] -eq 'term-cards' -and ($A -contains 'check')) { Write-Output '{"ok":true}'; exit 0 }
    if ($A[0] -eq 'term-cards' -and ($A -contains 'review-plan')) { Write-Output '{"ok":true,"jobs":[]}'; exit 0 }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'done') {
        Write-Output '{"kind":"ledger_done_batch","ok":false,"stop_reason":"readiness_changed","done":1,"not_run":1,"results":[{"unit":"termcards/t001@1","token":1,"status":"done"},{"unit":"termcards/t002@1","token":2,"status":"not_run","code":"readiness_changed"}]}'
        exit 0
    }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'status') {
        Write-Output '{"units":[{"id":"termcards/t001@1","state":"done","token":1},{"id":"termcards/t002@1","state":"claimed","token":2}]}'
        exit 0
    }
}
if ($mode -eq 'termlink_live_claim') {
    if ($A[0] -eq 'term-link' -and ($A -contains 'plan')) { Write-Output '{"ok":true,"cards_changed":1}'; exit 0 }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'next') { Write-Output '{"units":[]}'; exit 0 }
    if ($A[0] -eq 'ledger' -and $A[1] -eq 'gate') { Write-Output '{"complete":false,"counts":{"claimed":1,"open":1}}'; exit 3 }
    Write-Output '{"error":"should_not_apply_while_claimed"}'
    exit 1
}
Write-Output '{"ok":true}'
exit 0
'@ | Set-Content -LiteralPath $mockPs1 -Encoding utf8

    $env:MOCK_LOG = $mockLog
    $env:MOCK_STATE = Join-Path $scratch 'mock-state.txt'

    # Probe A: Cards budget continuation drains remaining via -UnitsFile before next -All
    $env:MOCK_MODE = 'cards_budget'
    Remove-Item -LiteralPath $mockLog, $env:MOCK_STATE -ErrorAction SilentlyContinue
    $rA = (& pwsh -NoProfile -File (Join-Path $scripts 'layer-steps.ps1') -Step start -Out $mockOut -Layer cards -Owner 'cards-test' -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    $logA = Get-Content -LiteralPath $mockLog
    $idxUnits = ($logA | Select-String -Pattern '-UnitsFile').LineNumber | Select-Object -First 1
    $allLines = @($logA | Select-String -Pattern '-Layer cards -All')
    Check ($rA.ok -eq $true -and $rA.done -eq 2 -and $idxUnits -and $allLines.Count -eq 2 -and $idxUnits -lt $allLines[1].LineNumber) "Cards budget continuation resumes remaining via -UnitsFile BEFORE calling -All again"

    # Probe B: Event native apply Lock stops BEFORE report or finish-batch
    $env:MOCK_MODE = 'event_apply_lock'
    Remove-Item -LiteralPath $mockLog -ErrorAction SilentlyContinue
    Save-Json (Join-Path $mockOut 'layer-steps\events\units.json') @(@{ id = 'events/g001@1'; token = 1 })
    $rB = (& pwsh -NoProfile -File (Join-Path $scripts 'layer-steps.ps1') -Step finish -Out $mockOut -Layer events -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    $logB = Get-Content -LiteralPath $mockLog
    Check ($rB.ok -eq $false -and $rB.code -eq 'card_locked' -and -not ($logB -match 'report') -and -not ($logB -match 'finish-batch')) "Event native apply Lock stops immediately BEFORE report or finish-batch"

    # Probe C: model-steps scope_refused preserved vs untyped exit 2 vs transport stop
    Save-Json (Join-Path $mockOut 'models\drive\units.json') @(@{ id = 'models/g001-m0000@1'; token = 1 }, @{ id = 'models/g002-m0000@1'; token = 2 })
    $env:MOCK_MODE = 'scope_refused_finish'
    $rC1 = (& pwsh -NoProfile -File (Join-Path $scripts 'model-steps.ps1') -Step finish -Out $mockOut -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    Check ($rC1.ok -eq $false -and $rC1.code -eq 'scope_refused') "model-steps finish preserves code=scope_refused (not rewritten to card_locked)"

    $env:MOCK_MODE = 'untyped_exit2'
    $rC2 = (& pwsh -NoProfile -File (Join-Path $scripts 'model-steps.ps1') -Step finish -Out $mockOut -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    Check ($rC2.ok -eq $false -and "$($rC2.code)" -ne 'card_locked') "model-steps finish does NOT misclassify untyped exit 2 as card_locked"

    $env:MOCK_MODE = 'transport_stop'
    $rC3 = (& pwsh -NoProfile -File (Join-Path $scripts 'model-steps.ps1') -Step finish -Out $mockOut -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    Check ($rC3.ok -eq $false -and "$($rC3.error)" -match 'transport') "model-steps finish fails on stop_reason=transport instead of dropping not_run"

    # Probe D: term-steps truncated/wrong-kind done_batch reconciles and fails step without calling ledger fail
    $env:MOCK_MODE = 'term_truncated_done'
    Remove-Item -LiteralPath $mockLog -ErrorAction SilentlyContinue
    Save-Json (Join-Path $mockOut 'term-cards\drive\units.json') @(
        @{ id = 'termcards/t001@1'; token = 1; owner = 'term cards'; inputs = @{ job = @{ id = 't001' } } },
        @{ id = 'termcards/t002@1'; token = 2; owner = 'term cards'; inputs = @{ job = @{ id = 't002' } } }
    )
    $rD = (& pwsh -NoProfile -File (Join-Path $scripts 'term-steps.ps1') -Step finish -Out $mockOut -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    $logD = Get-Content -LiteralPath $mockLog
    Check ($rD.ok -eq $false -and $rD.step -eq 'done' -and -not ($logD -match 'ledger fail')) "term-steps truncated/wrong-kind done_batch reconciles and fails step without faulting units"

    # Probe E: term-steps -Retry handles after_commit (ledger_state=done), passes -Owner, preserves blocked without ledger fail
    $env:MOCK_MODE = 'term_retry_mixed'
    Remove-Item -LiteralPath $mockLog -ErrorAction SilentlyContinue
    Save-Json (Join-Path $mockOut 'term-cards\drive\units.json') @(
        @{ id = 'termcards/t001@1'; token = 1; owner = 'term cards'; inputs = @{ job = @{ id = 't001' } } },
        @{ id = 'termcards/t002@1'; token = 2; owner = 'term cards'; inputs = @{ job = @{ id = 't002' } } },
        @{ id = 'termcards/t003@1'; token = 3; owner = 'term cards'; inputs = @{ job = @{ id = 't003' } } }
    )
    $rE = (& pwsh -NoProfile -File (Join-Path $scripts 'term-steps.ps1') -Step finish -Retry -Out $mockOut -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    $logE = Get-Content -LiteralPath $mockLog
    Check ($rE.ok -eq $true -and $rE.done -eq 1 -and $rE.retry -eq 1 -and $rE.blocked -eq 1 -and -not ($logE -match 'ledger fail') -and ($logE -match '-Owner term cards')) "term-steps -Retry counts after_commit as done, passes -Owner, preserves blocked without ledger fail"

    # Probe F: term-steps stops without faulting healthy jobs when done_batch reports stop_reason=readiness_changed
    $env:MOCK_MODE = 'term_readiness_changed'
    Remove-Item -LiteralPath $mockLog -ErrorAction SilentlyContinue
    Save-Json (Join-Path $mockOut 'term-cards\drive\units.json') @(
        @{ id = 'termcards/t001@1'; token = 1; owner = 'term cards'; inputs = @{ job = @{ id = 't001' } } },
        @{ id = 'termcards/t002@1'; token = 2; owner = 'term cards'; inputs = @{ job = @{ id = 't002' } } }
    )
    $rF = (& pwsh -NoProfile -File (Join-Path $scripts 'term-steps.ps1') -Step finish -Out $mockOut -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    $logF = Get-Content -LiteralPath $mockLog
    Check ($rF.ok -eq $false -and $rF.step -eq 'done' -and -not ($logF -match 'ledger fail')) "term-steps preserves remaining claims on stop_reason=readiness_changed without calling ledger fail"

    # Probe G: term-steps link refuses unclaimed apply while another worker holds a live claim
    $env:MOCK_MODE = 'termlink_live_claim'
    Remove-Item -LiteralPath $mockLog -ErrorAction SilentlyContinue
    $rG = (& pwsh -NoProfile -File (Join-Path $scripts 'term-steps.ps1') -Step link -Out $mockOut -Tool $mockCmd | Select-Object -Last 1) | ConvertFrom-Json
    $logG = Get-Content -LiteralPath $mockLog
    Check ($rG.ok -eq $false -and $rG.step -eq 'link' -and -not ($logG -match '-Step apply')) "term-steps link refuses unclaimed apply while another worker holds a live claim"
} finally {
    try { zt down | Out-Null } catch {}
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "`nbatch-acceleration: $pass passed, $fail failed"
if ($fail -gt 0) { exit 1 }
