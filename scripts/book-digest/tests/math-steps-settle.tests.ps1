#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\math-steps.tests.ps1
# math-steps.ps1 settles each mathematics layer through the ledger CLI (docs/math-digest-redesign-proposal.md,
# Bug D). A fake `zt-digest` (-Tool) records every call and answers as the real CLI does, so the test pins the
# CLI contract: claims of at most 64, drained page by page, completed with the layer's own evidence file, and a
# refused claim that stops the step instead of reading as zero work. No archive and no provider is touched.
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$steps = Join-Path $PSScriptRoot '..\math-steps.ps1'
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "math-steps-test-$PID"))
function Assert($condition, $message) { if (-not $condition) { throw "FAIL: $message" }; Write-Output "ok: $message" }

# The fake CLI: `math -Step <s>` answers the step's line; `ledger next` hands out the queued units of the layer
# (at most -Take, refusing more than 64 as the real CLI does); `ledger done` and `ledger fail` succeed.
$fake = Join-Path $scratch 'fake-zt-digest.ps1'
New-Item -ItemType Directory -Force $scratch | Out-Null
@'
$log = $env:FAKE_LOG
Add-Content -LiteralPath $log -Value ($args -join ' ') -Encoding utf8
$a = @($args)
$get = { param($name) $i = [array]::IndexOf($a, "-$name"); if ($i -ge 0) { $a[$i + 1] } else { $null } }
if ($a[0] -eq 'math') {
    switch (& $get 'Step') {
        'check' { '{"ok":true,"units":2,"theories":1,"faults":0,"by_rule":{},"report":"r.md"}' }
        'notation-apply' { '{"ok":true,"symbols":3,"card":"k/1|a"}' }
        'proof-apply' { '{"ok":2,"fixed":0,"refused":0,"parts":[]}' }
        default { '{"ok":true}' }
    }
    exit 0
}
if ($a[0] -eq 'ledger' -and $a[1] -eq 'next') {
    if ($env:FAKE_NEXT_FAIL) { 'error: the ledger is locked'; exit 1 }
    $take = [int](& $get 'Take')
    if ($take -lt 1 -or $take -gt 64) { 'Error: -Take must be a positive integer from 1 through 64'; exit 1 }
    $layer = & $get 'Layer'
    $queue = Join-Path $env:FAKE_DIR "queue-$layer.txt"
    $ids = @(if (Test-Path $queue) { Get-Content $queue | Where-Object { $_ } })
    $page = @($ids | Select-Object -First $take)
    Set-Content -LiteralPath $queue -Value @($ids | Select-Object -Skip $page.Count) -Encoding utf8
    foreach ($id in $page) { Add-Content -LiteralPath $log -Value "claim $id" -Encoding utf8 }
    $units = @($page | ForEach-Object { @{ id = $_; token = 7 } })
    ConvertTo-Json -Compress -InputObject @{ units = $units } -Depth 5
    exit 0
}
if ($a[0] -eq 'ledger' -and $a[1] -eq 'status') {
    $layer = & $get 'Layer'
    if ($env:FAKE_STATUS) { $env:FAKE_STATUS; exit 0 }
    ConvertTo-Json -Compress -Depth 5 -InputObject @{ counts = @{ per_layer = @{} } }
    exit 0
}
if ($a[0] -eq 'ledger' -and $a[1] -eq 'reopen') {
    # As the real CLI: a done unit is reopened only with -Recheck.
    if ($a -notcontains '-Recheck') { 'zt-digest ledger: unit is done, not failed: only a failed unit is reopened'; exit 1 }
    '{"reopened":1}'; exit 0
}
if ($a[0] -eq 'ledger' -and $a[1] -eq 'fail') {
    if ($env:FAKE_FAIL_REFUSE -and (& $get 'Unit') -like $env:FAKE_FAIL_REFUSE) { 'error: the ledger is busy'; exit 1 }
    # As the real CLI: -Fault is a file path or the fault document as JSON text (thompson pilot 2026-11-22:
    # a plain-text fault was read as a missing path and the step stopped).
    try { $doc = (& $get 'Fault') | ConvertFrom-Json -ErrorAction Stop } catch { "zt-digest ledger: Cannot find path '$(& $get 'Fault')'"; exit 1 }
    if ($doc.schema_version -ne 1 -or -not $doc.cause -or $doc.unit -ne (& $get 'Unit')) { 'zt-digest ledger: invalid fault document'; exit 1 }
    # A retryable failure makes the unit claimable again, as the real ledger does after its short backoff.
    if ($env:FAKE_REQUEUE) {
        $unit = & $get 'Unit'
        $layer = ($unit -split '/')[0]
        Add-Content -LiteralPath (Join-Path $env:FAKE_DIR "queue-$layer.txt") -Value $unit -Encoding utf8
    }
    '{"state":"ready"}'; exit 0
}
if ($a[0] -eq 'ledger' -and $a[1] -eq 'done') {
    if ($env:FAKE_DONE_REFUSE -and (& $get 'Unit') -like $env:FAKE_DONE_REFUSE) { 'error: the proof of k/5|a has no proof step'; exit 1 }
    '{"state":"done"}'; exit 0
}
'{"ok":true}'; exit 0
'@ | Set-Content -LiteralPath $fake -Encoding utf8

function Invoke-Step([string]$Step, [hashtable]$Queues = @{}, [hashtable]$Env = @{}) {
    $dir = Join-Path $scratch ([guid]::NewGuid().ToString('n'))
    $out = Join-Path $dir 'staging'
    New-Item -ItemType Directory -Force (Join-Path $out 'math\notation'), (Join-Path $out 'math\proof'), (Join-Path $out 'math\boundary') | Out-Null
    Set-Content (Join-Path $out 'math-audit.json') '{}' -Encoding utf8
    Set-Content (Join-Path $out 'math\notation\applied.json') '{}' -Encoding utf8
    Set-Content (Join-Path $out 'math\proof\applied.json') '{}' -Encoding utf8
    Set-Content (Join-Path $out 'math\boundary\settled.json') '{}' -Encoding utf8
    foreach ($f in 'index.json', 'docs.json', 'units.json') { Set-Content (Join-Path $out "math\$f") '{}' -Encoding utf8 }
    foreach ($k in $Queues.Keys) { Set-Content (Join-Path $dir "queue-$k.txt") $Queues[$k] -Encoding utf8 }
    $env:FAKE_DIR = $dir
    $env:FAKE_LOG = Join-Path $dir 'calls.log'
    $env:FAKE_NEXT_FAIL = $Env.NEXT_FAIL
    $env:FAKE_DONE_REFUSE = $Env.DONE_REFUSE
    $env:FAKE_REQUEUE = $Env.REQUEUE
    $env:FAKE_FAIL_REFUSE = $Env.FAIL_REFUSE
    $env:FAKE_STATUS = $Env.STATUS
    $extra = @(if ($Env.LAYER) { '-Layer', $Env.LAYER })
    if ($Env.NO_EVIDENCE) { Remove-Item -LiteralPath (Join-Path $out $Env.NO_EVIDENCE) }
    $lines = @(& pwsh -NoProfile -File $steps -Step $Step -Out $out -Tool $fake @extra 2>&1)
    $line = @($lines | Where-Object { "$_".Trim().StartsWith('{') }) | Select-Object -Last 1
    [pscustomobject]@{ json = ($line | ConvertFrom-Json); calls = @(if (Test-Path -LiteralPath $env:FAKE_LOG) { Get-Content $env:FAKE_LOG }); out = $out }
}

try {
    # The math check settles math-check/book@1 with the book's math audit.
    $r = Invoke-Step 'check' @{ 'math-check' = @('math-check/book@1') }
    Assert ($r.json.ok -eq $true -and $r.json.settled -eq 1) 'check settles its one ledger unit'
    $done = @($r.calls | Where-Object { $_ -like 'ledger done*' })
    Assert ($done.Count -eq 1 -and $done[0] -like "*-Outputs $(Join-Path $r.out 'math-audit.json')") 'the math check completes with <out>\math-audit.json'
    Assert (@($r.calls | Where-Object { $_ -like 'ledger next*' -and $_ -match '-Take (\d+)' -and [int]$Matches[1] -gt 64 }).Count -eq 0) 'no claim asks for more than 64 units'

    # Notation completes with math\notation\applied.json.
    $r = Invoke-Step 'notation-apply' @{ 'notation' = @('notation/book@1') }
    Assert ($r.json.settled -eq 1 -and @($r.calls | Where-Object { $_ -like "*-Outputs $(Join-Path $r.out 'math\notation\applied.json')" }).Count -eq 1) 'notation completes with math\notation\applied.json'

    # Proof: 70 ready obligations are drained in pages of at most 64; a refused one is failed and named.
    $ids = @(1..70 | ForEach-Object { "proof/k-$_@1" })
    $r = Invoke-Step 'proof-apply' @{ 'proof' = $ids } @{ DONE_REFUSE = 'proof/k-3@1' }
    Assert ($r.json.settled -eq 69) 'all 70 proof obligations are claimed across pages and 69 settle'
    Assert (@($r.calls | Where-Object { $_ -like "*-Outputs $(Join-Path $r.out 'math\proof\applied.json')" }).Count -eq 70) 'proof completes with math\proof\applied.json'
    Assert ($r.json.settle_refused -eq 1 -and @($r.calls | Where-Object { $_ -like 'ledger fail*proof/k-3@1*' }).Count -eq 1) 'the refused obligation is failed and counted'
    # The ledger classifies the fault from its cause; a forced class it does not permit refuses the fail (pilot: proof/5|a).
    Assert (-not @($r.calls | Where-Object { $_ -like 'ledger fail*' -and $_ -like '*-Class*' }).Count) 'a refused unit is failed without a forced class'
    $detail = Get-Content -Raw (Join-Path $r.out 'math\drive-lines\proof-apply.json') | ConvertFrom-Json
    Assert ("$($detail.detail.settle.refused)" -like '*proof/k-3@1*') 'the detail names each refused unit and its reason'

    # A refused unit that is claimable again is never left claimed: every claim ends in done or fail, and the
    # drain still settles the others and stops (Astra round 6).
    $ids = @(1..5 | ForEach-Object { "proof/k-$_@1" })
    $r = Invoke-Step 'proof-apply' @{ 'proof' = $ids } @{ DONE_REFUSE = 'proof/k-2@1'; REQUEUE = '1' }
    Assert ($r.json.settled -eq 4) 'the four other obligations settle while one keeps being refused'
    $claims = @($r.calls | Where-Object { $_ -like 'claim proof/k-2@1' }).Count
    $closed = @($r.calls | Where-Object { ($_ -like 'ledger done*proof/k-2@1*') -or ($_ -like 'ledger fail*proof/k-2@1*') }).Count
    Assert ($claims -ge 2 -and $claims -eq @($r.calls | Where-Object { $_ -like 'ledger fail*proof/k-2@1*' }).Count) 'every claim of the refused unit is failed again, none is left held'

    # A `ledger fail` that errors stops the step, but only after the rest of the claimed page is closed (Astra round 7).
    $ids = @(1..4 | ForEach-Object { "proof/k-$_@1" })
    $r = Invoke-Step 'proof-apply' @{ 'proof' = $ids } @{ DONE_REFUSE = 'proof/k-1@1'; FAIL_REFUSE = 'proof/k-1@1' }
    Assert ($r.json.ok -eq $false) 'a failed ledger fail stops the step'
    Assert (@(2..4 | Where-Object { $n = $_; @($r.calls | Where-Object { $_ -like "ledger done*proof/k-$n@1*" }).Count -eq 1 }).Count -eq 3) 'the rest of the page is still completed'

    # Boundary completes with math\boundary\settled.json.
    $r = Invoke-Step 'boundary-settle' @{ 'boundary' = @('boundary/k-1@1') }
    Assert ($r.json.settled -eq 1 -and @($r.calls | Where-Object { $_ -like "*-Outputs $(Join-Path $r.out 'math\boundary\settled.json')" }).Count -eq 1) 'boundary completes with math\boundary\settled.json'

    # The skeleton settles index, docs and skeleton in order, then registers the unit builds (`ledger math`).
    $r = Invoke-Step 'skeleton' @{ 'math-index' = @('math-index/book@1'); 'math-docs' = @('math-docs/book@1'); 'math-skeleton' = @('math-skeleton/book@1') }
    $order = @($r.calls | Where-Object { $_ -like 'ledger done*' -or $_ -like 'ledger math*' } | ForEach-Object { ($_ -split ' ')[1..3] -join ' ' })
    Assert ($r.json.ok -eq $true -and $r.json.settled -eq 3) 'the skeleton settles its three book layers'
    $doneSkeleton = [array]::FindIndex([string[]]$r.calls, [Predicate[string]] { param($c) $c -like 'ledger done*math-skeleton/book@1*' })
    $register = [array]::FindIndex([string[]]$r.calls, [Predicate[string]] { param($c) $c -like 'ledger math*' })
    Assert ($doneSkeleton -ge 0 -and $register -gt $doneSkeleton) 'the unit builds are registered after the skeleton settles'

    # A rerun of the check measures the archive now: a settled math-check unit is reopened, then settled again with the
    # fresh audit (thompson pilot: mode math reran the check and the run report kept the old blocking count).
    $doneCheck = '{"counts":{"per_layer":{"math-check":{"total":1,"done":1,"open":0}}}}'
    $r = Invoke-Step 'check' @{ 'math-check' = @('math-check/book@1') } @{ STATUS = $doneCheck }
    $reopen = [array]::FindIndex([string[]]$r.calls, [Predicate[string]] { param($c) $c -like 'ledger reopen*math-check/book@1*' })
    $settled = [array]::FindIndex([string[]]$r.calls, [Predicate[string]] { param($c) $c -like 'ledger done*math-check/book@1*' })
    Assert ($reopen -ge 0 -and $settled -gt $reopen -and $r.json.settled -eq 1) 'check: a settled math-check unit is reopened and settled with the fresh audit'

    # A resume reads a math layer's ledger state (thompson pilot 2026-11-22: a resume re-planned the boundary stage and
    # the ledger refused to rebind its open unit): registered, done and open obligations, and whether its evidence is there.
    # The ledger's per-layer `open` leaves ready units out (pilot: 2|e ready, open 0): open is what is neither done nor failed.
    $status = '{"counts":{"per_layer":{"boundary":{"total":13,"done":12,"noop":0,"superseded":0,"failed":0,"ready":1,"open":0}}}}'
    $r = Invoke-Step 'layer-state' @{} @{ LAYER = 'boundary'; STATUS = $status }
    Assert ($r.json.ok -eq $true -and $r.json.total -eq 13 -and $r.json.done -eq 12 -and $r.json.open -eq 1 -and $r.json.evidence -eq $true) 'layer-state: the boundary obligations and their evidence'
    $r = Invoke-Step 'layer-state' @{} @{ LAYER = 'proof'; STATUS = $status; NO_EVIDENCE = 'math\proof\applied.json' }
    Assert ($r.json.ok -eq $true -and -not $r.json.total -and -not $r.json.evidence) 'layer-state: a layer the ledger does not hold is 0 with no evidence (the compact line drops 0 and false)'
    # The settle alone completes the open obligations with the layer's own evidence, and runs no compiler step.
    $r = Invoke-Step 'settle' @{ 'boundary' = @('boundary/k-5@1') } @{ LAYER = 'boundary' }
    Assert ($r.json.ok -eq $true -and $r.json.settled -eq 1 -and -not @($r.calls | Where-Object { $_ -like 'math *' }).Count) 'settle: the open boundary unit settles without a compiler step'
    $r = Invoke-Step 'settle' @{ 'proof' = @('proof/k-5@1') } @{ LAYER = 'proof' }
    Assert (@($r.calls | Where-Object { $_ -like "*-Outputs $(Join-Path $r.out 'math\proof\applied.json')" }).Count -eq 1) 'settle: proof settles with math\proof\applied.json'

    # A claim the CLI refuses stops the step: it never reads as a layer with nothing to settle.
    $r = Invoke-Step 'check' @{} @{ NEXT_FAIL = '1' }
    Assert ($r.json.ok -eq $false -and "$($r.json.error)" -like '*claim*') 'a refused claim fails the step'
    Write-Output 'all passed'
} finally {
    Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue
}
