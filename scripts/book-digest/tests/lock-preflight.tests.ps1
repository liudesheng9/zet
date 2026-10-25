#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\lock-preflight.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# Tree Lock S14 (#137): a digest write mode stops before its first write and before it allocates a
# job when the book's Literature tree is locked by the owner, while a read-only audit, checker or
# report of the same tree still reads it. A scratch archive with its own service; the lock is a real
# owner Lock taken through the GUI API. The live archive is never touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "lock-preflight-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
$archive = Join-Path $scratch 'archive'
New-Item -ItemType Directory -Force $archive | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$here = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $here 'zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Invoke-Tool([string[]]$arguments) {
    $lines = @(& $Digest @arguments 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1]; text = ($lines -join ' ') }
}
function Invoke-Script($name, [string[]]$arguments) {
    $lines = @(pwsh -NoProfile -File (Join-Path $here $name) @arguments 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1]; text = ($lines -join ' ') }
}
# The GUI of this scratch service: its URL and the token its page carries.
function Get-Gui {
    $line = @(@(zt gui 2>&1) | Where-Object { $_ -match '^zt gui: http' })[0]
    if (-not $line) { throw 'zt gui printed no URL' }
    $url = (($line -replace '^zt gui: ', '') -replace '/+$', '')
    $page = (Invoke-WebRequest -UseBasicParsing -Uri $url).Content
    $token = [regex]::Match($page, 'name="zt-token" content="([^"]+)"').Groups[1].Value
    if (-not $token) { throw 'the served page carries no token' }
    [pscustomobject]@{ url = $url; token = $token }
}
function Invoke-Gui($gui, [string]$path, $body) {
    Invoke-RestMethod -Method Post -Uri "$($gui.url)$path" -Headers @{ 'X-ZT-Token' = $gui.token } -ContentType 'application/json' -Body ($body | ConvertTo-Json -Depth 8)
}
# The owner locks one whole tree: plan, then the plan that carries its own hash.
function Lock-Tree([string]$root, [string]$note = 'reviewed') {
    $gui = Get-Gui
    $plan = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note }
    if (-not $plan.plan.plan_hash) { throw "the lock plan carries no plan_hash: $($plan | ConvertTo-Json -Depth 8 -Compress)" }
    $done = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note; confirm = 'lock'; plan_hash = $plan.plan.plan_hash }
    if (-not $done.applied) { throw "the owner lock was not applied: $($done | ConvertTo-Json -Depth 8 -Compress)" }
    $done.lock
}
function Refused-Locked($run, [string]$key, [string]$what) {
    $message = "the Literature tree $key is locked by the owner"
    Assert ($run.code -ne 0) "$what stops the step (exit $($run.code))"
    Assert ($run.text.Contains($message)) "$what names the exact contract: $($run.text)"
    $typed = $null
    foreach ($line in $run.lines) {
        if (-not $line.TrimStart().StartsWith('{')) { continue }
        try { $candidate = $line | ConvertFrom-Json } catch { continue }
        if ($candidate.code) { $typed = $candidate }
    }
    Assert ($typed -and $typed.code -eq 'card_locked' -and $typed.tree -eq $key -and $typed.lock_id -gt 0) "$what also answers the typed card_locked JSON: $($run.lines -join ' | ')"
}

try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null

    # --- two books, one locked ------------------------------------------------------------------------
    $book = @'
# 导论

## 导论.1 企业为什么存在

企业存在是因为市场失灵。1937 年科斯提出了这个问题。

# 第一章 契约

## 1.1 不完全契约

契约不完全，剩余控制权归所有者，企业投资随之改变。

# 参考

文献。
'@
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, $book, $utf8)
    function New-Book([string]$key, [string]$staging) {
        $run = Invoke-Script 'prepare-book-digest.ps1' @('-Source', $source, '-Out', $staging, '-CitationKey', $key,
            '-BibTeX', "@book{$key, title={企业与契约}}", '-BookTitle', '企业与契约', '-SkipHeadings', '习题',
            '-MinUnitChars', '20', '-MaxUnits', '1', '-NoFigures', '-Apply')
        if ($run.code -ne 0) { throw "prepare $key failed: $($run.text)" }
        $run.last | ConvertFrom-Json
    }
    $locked = New-Book 'Locked2021' (Join-Path $scratch 'locked-book')
    $open = New-Book 'Open2020' (Join-Path $scratch 'open-book')

    $lock = Lock-Tree 'Locked2021' 'reviewed by the owner'
    Assert ($lock.lock_id -gt 0 -and $lock.tree -eq 'Locked2021') "the owner locks the whole Literature tree (lock $($lock.lock_id))"

    # --- the read-only modes still read the locked tree (R327) ---------------------------------------
    $audit = Invoke-Script 'digest-audit.ps1' @('-CitationKey', 'Locked2021', '-Out', (Join-Path $scratch 'audit'))
    Assert ($audit.code -eq 0) "the audit reads the locked tree normally: $($audit.text)"
    $models = Invoke-Tool @('list-models', '-Staging', $locked.staging, '-All', '-CitationKey', 'Locked2021')
    Assert ($models.code -eq 0) "the model report reads the locked tree normally: $($models.text)"

    # --- a full digest into the locked Literature tree stops (R329/R334) ------------------------------
    $again = Invoke-Script 'prepare-book-digest.ps1' @('-Source', $source, '-Out', (Join-Path $scratch 'locked-book-again'),
        '-CitationKey', 'Locked2021', '-BibTeX', '@book{Locked2021, title={企业与契约}}', '-BookTitle', '企业与契约',
        '-MinUnitChars', '20', '-MaxUnits', '1', '-NoFigures', '-Apply')
    Refused-Locked $again 'Locked2021' 'a second prepare of the same locked book'
    Assert (-not (Test-Path -LiteralPath (Join-Path $scratch 'locked-book-again'))) 'the stopped prepare wrote no staging folder'

    # --- the helper keeps the typed refusal and the earlier committed receipt (R338) -----------------
    $helper = Join-Path $PSScriptRoot '..\..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path
    $log = Join-Path $scratch 'helper-owned.txt'
    $batch = @(
        @{ tool = 'create_topic'; arguments = @{ title = '企业边界'; body = '本主题追问：什么决定企业的边界。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Locked2021/1'; kind = 'side'; title = '锁定树里的新卡'; body = '这张卡写不进去。' } }
    )
    $said = @(($batch | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $helper - -Open -Log $log | ForEach-Object { "$_" })
    $saidExit = $LASTEXITCODE
    Assert ($saidExit -ne 0) "a write into the locked tree stops the helper (exit $saidExit): $($said -join ' | ')"
    Assert (@($said | Where-Object { $_ -match '^0 ok \d+/0$' }).Count -eq 1) "the earlier Card of the batch stays committed: $($said -join ' | ')"
    Assert (@($said | Where-Object { $_ -match '^1 error card_locked' }).Count -eq 1) "the refused call says card_locked: $($said -join ' | ')"
    $typedLine = @($said | Where-Object { $_ -match '^card_locked ' })[0]
    Assert ([bool]$typedLine) "the helper also prints the refusal as JSON: $($said -join ' | ')"
    if ($typedLine) {
        $refusal = ($typedLine -replace '^card_locked ', '') | ConvertFrom-Json
        Assert ($refusal.error -eq 'card_locked' -and $refusal.tree -eq 'Locked2021' -and $refusal.reason -eq 'create-inside' -and $refusal.locks[0].lock_id -gt 0 -and $refusal.locks[0].tree -eq 'Locked2021') "the JSON keeps the MCP refusal's own fields: $typedLine"
    }
    Assert (@($said | Where-Object { $_ -match '^resume: -From 1 -Addresses \d+/0$' }).Count -eq 1) "the resume line names the refused call: $($said -join ' | ')"
    Assert (@(Get-Content -LiteralPath $log -Encoding utf8).Count -eq 1) 'the earlier committed address stays in the write log'

    # The PowerShell caller keeps the same typed refusal: Invoke-Zt throws with the JSON itself, so a
    # script reads the tree, Lock id and reason instead of an English sentence.
    $scriptSaid = ''
    try {
        $null = Invoke-Zt @(@{ tool = 'create_successor'; arguments = @{ at = 'Locked2021/1'; kind = 'side'; title = '脚本里的写入'; body = '这张卡写不进去。' } })
    } catch { $scriptSaid = "$_" }
    Assert ($scriptSaid -match 'card_locked' -and $scriptSaid -match '"tree":"Locked2021"' -and $scriptSaid -match '"reason"') "the PowerShell helper keeps the typed refusal: $scriptSaid"

    # --- every writing mode stops before it allocates a job or writes (R328) --------------------------
    $openItem = Get-Content -LiteralPath (Join-Path $open.staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
    $lockedItem = Get-Content -LiteralPath (Join-Path $locked.staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json

    # A locked Card stays readable by the checker and linkable from another tree: the Lock stops
    # writes into its tree, never reading or linking (R327/§15.2's agent policy).
    $check = @(& (Get-Command zt-check -CommandType Application).Source --cards 'Locked2021/1' --book Locked2021 2>&1 | ForEach-Object { "$_" })
    Assert ($LASTEXITCODE -ne 2 -and ($check -join ' ') -match 'faults: \d+') "the checker reads a locked Card normally: $($check -join ' ')"
    $thought = @(
        @{ tool = 'create_topic'; arguments = @{ title = '锁定之后的新想法'; body = '本主题追问：锁定卡还能不能被引用。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '锁定卡仍然可以链接'; body = "这张卡用 [[Locked2021/1]] 说明：锁定树里的卡片可以读、可以链接。`n`n来源：[[Locked2021]]" } }
    )
    $linked = @(($thought | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $helper - -Open | ForEach-Object { "$_" })
    $linkedExit = $LASTEXITCODE
    Assert ($linkedExit -eq 0) "a new thought in another unlocked tree still links a locked Card: $($linked -join ' | ')"

    $skeletonWork = Join-Path $scratch 'skeleton-work'
    $skeleton = Invoke-Tool @('skeleton', '-Step', 'plan', '-Staging', $locked.staging, '-Work', $skeletonWork)
    Refused-Locked $skeleton 'Locked2021' 'the skeleton plan'
    Assert (-not (Test-Path -LiteralPath (Join-Path $skeletonWork 'job.md'))) 'the stopped skeleton plan wrote no job'

    $openSkeleton = Join-Path $scratch 'open-skeleton'
    $skeletonOpen = Invoke-Tool @('skeleton', '-Step', 'plan', '-Staging', $open.staging, '-Work', $openSkeleton)
    Assert ($skeletonOpen.code -eq 0 -and (Test-Path -LiteralPath (Join-Path $openSkeleton 'job.md'))) "an unlocked book still allocates its skeleton job while another tree is locked: $($skeletonOpen.text)"

    $termsWork = Invoke-Tool @('terms', '-Step', 'plan', '-Staging', $locked.staging)
    Refused-Locked $termsWork 'Locked2021' 'the Terms plan'
    Assert (-not (Test-Path -LiteralPath (Join-Path $locked.staging 'terms'))) 'the stopped Terms plan wrote no folder'

    $events = Invoke-Tool @('event', '-Step', 'plan', '-Staging', $locked.staging)
    Refused-Locked $events 'Locked2021' 'the Event plan'
    $findings = Invoke-Tool @('finding', '-Step', 'plan', '-Staging', $locked.staging)
    Refused-Locked $findings 'Locked2021' 'the Finding plan'
    Assert (-not (Test-Path -LiteralPath (Join-Path $locked.staging 'events'))) 'the stopped Event plan wrote no folder'
    Assert (-not (Test-Path -LiteralPath (Join-Path $locked.staging 'findings'))) 'the stopped Finding plan wrote no folder'

    $mathIndex = Invoke-Tool @('math', '-Step', 'index', '-Staging', $locked.staging)
    Assert ($mathIndex.code -eq 0) "the local math index of a locked book still runs (no Card, no archive read): $($mathIndex.text)"
    $math = Invoke-Tool @('math', '-Step', 'skeleton', '-Staging', $locked.staging)
    Refused-Locked $math 'Locked2021' 'the math skeleton'
    Assert (-not (Test-Path -LiteralPath (Join-Path $locked.staging 'math\units.json'))) 'the stopped math skeleton wrote no unit plan'
    $cardWork = Join-Path $scratch 'card-work'
    $cards = Invoke-Tool @('card', '-Step', 'plan', '-Work', $cardWork, '-Book', 'Locked2021', '-Staging', $locked.staging)
    Refused-Locked $cards 'Locked2021' 'the card cycle plan'
    Assert (-not (Test-Path -LiteralPath $cardWork)) 'the stopped card plan made no work folder'
    $modelWork = Join-Path $scratch 'model-work'
    $models = Invoke-Tool @('model', '-Step', 'brief', '-Work', $modelWork, '-Staging', $locked.staging, '-Models', 'Locked2021/1')
    Refused-Locked $models 'Locked2021' 'the model brief'
    Assert (-not (Test-Path -LiteralPath $modelWork)) 'the stopped model brief made no work folder'

    $figuresFile = Join-Path $scratch 'figures.json'
    [IO.File]::WriteAllText($figuresFile, '{"file_id":0,"figures":[{"name":"图 1.1","labels":["1.1"],"alt":"","line":"","sha256":"","path":""}]}', $utf8)
    $figures = Invoke-Tool @('figures', '-CitationKey', 'Locked2021', '-FiguresFile', $figuresFile, '-Report', (Join-Path $scratch 'figures-report.json'), '-Apply')
    Refused-Locked $figures 'Locked2021' 'the figures apply'

    $link = Invoke-Tool @('prepare-link', '-Staging', $locked.staging)
    Refused-Locked $link 'Locked2021' 'the link-job preparation'
    Assert (-not (Test-Path -LiteralPath (Join-Path $locked.staging 'cards-all.md'))) 'the stopped link preparation wrote no job files'

    # --- the resume of a prepared run stops too (R328: it dispatches the same book's work) ------------
    $variantOut = Join-Path $scratch 'variant-book'
    $options = Join-Path $scratch 'options.json'
    [IO.File]::WriteAllText($options, '{"terms":true}', $utf8)
    $variant = Invoke-Script 'prepare-book-digest.ps1' @('-Source', $source, '-Out', $variantOut, '-CitationKey', 'Variant2022',
        '-BibTeX', '@book{Variant2022, title={第三本书}}', '-BookTitle', '第三本书', '-MinUnitChars', '20', '-MaxUnits', '1',
        '-NoFigures', '-Variant', 'econ', '-RunOptions', $options, '-Apply')
    Assert ($variant.code -eq 0) "a prepared ledger run is ready: $($variant.text)"
    $null = Lock-Tree 'Variant2022' 'the third book reviewed'
    $resume = Invoke-Tool @('ledger', 'resume', '-Out', $variantOut)
    Refused-Locked $resume 'Variant2022' 'the resume of a prepared run'
    $claimed = Invoke-Tool @('ledger', 'next', '-Out', $variantOut, '-Layer', 'terms', '-Take', '1', '-Owner', 'terms-manager')
    $token = 0
    try { $token = ($claimed.last | ConvertFrom-Json).units[0].token } catch { }
    if ($token -gt 0) {
        $claimedTerms = Invoke-Tool @('terms', '-Step', 'plan', '-Staging', $variantOut, '-Token', "$token")
        Refused-Locked $claimedTerms 'Variant2022' 'the claimed Terms plan'
    } else {
        Assert $false "a prepared run claims a Terms unit: $($claimed.text)"
    }

    # --- a tree that locks mid-run stops the write stage and keeps its drafts (R337, US041) -----------
    # The plan of the unlocked book allocated its job while the tree was free; the owner then locks
    # it. The apply stops at its precheck, the staged job is still there, and nothing was written.
    $midSkeleton = Join-Path $scratch 'mid-skeleton'
    $planned = Invoke-Tool @('skeleton', '-Step', 'plan', '-Staging', $open.staging, '-Work', $midSkeleton)
    Assert ($planned.code -eq 0 -and (Test-Path -LiteralPath (Join-Path $midSkeleton 'job.md'))) "the skeleton job is planned while the tree is free: $($planned.text)"
    $before = (Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = 'Open2020' } })).cards.Count
    $null = Lock-Tree 'Open2020' 'locked after its plan'
    [IO.File]::WriteAllText((Join-Path $midSkeleton 'out\skeleton.json'), '{"chapters":[],"argument":[],"models":[],"events":[],"topics":[]}', $utf8)
    $stoppedApply = Invoke-Tool @('skeleton', '-Step', 'apply', '-Staging', $open.staging, '-Work', $midSkeleton, '-Apply')
    Refused-Locked $stoppedApply 'Open2020' 'the apply of a job planned before the lock'
    Assert (Test-Path -LiteralPath (Join-Path $midSkeleton 'job.md')) 'the stopped apply keeps its staged job draft'
    Assert (-not $stoppedApply.text.Contains('"written"')) 'the stopped apply never reports itself as written or complete'
    $after = (Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = 'Open2020' } })).cards.Count
    Assert ($after -eq $before) "the stopped apply wrote no Card ($before cards before, $after after)"

    # A real mid-run race: the archive commits the run's own first call, the owner locks the tree
    # before its reply is sent, and the run's next write meets the real refusal. The seam is the
    # documented real-MCP proxy, so every answer below is the archive's own.
    $raceBook = New-Book 'MidRun2023' (Join-Path $scratch 'race-book')
    $seam = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $raceLockFlag = Join-Path $scratch 'race-lock.flag'
    $env:ZT_LOCK_TREE = 'MidRun2023'
    $env:ZT_LOCK_AFTER_TOOL = 'create_successor'
    $env:ZT_LOCK_FLAG = $raceLockFlag
    $env:ZT_CHECK_ZT = $seam
    $raceLog = Join-Path $scratch 'race-owned.txt'
    $race = @(
        @{ tool = 'create_successor'; arguments = @{ at = 'MidRun2023/1'; kind = 'side'; title = '锁定之前写入的卡片'; body = '这张卡片写入了。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'MidRun2023/1'; kind = 'side'; title = '锁定之后写入的卡片'; body = '这张卡片写不进去。' } }
    )
    $raced = @(($race | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $helper - -Open -Log $raceLog | ForEach-Object { "$_" })
    $racedExit = $LASTEXITCODE
    $env:ZT_CHECK_ZT = $null
    $env:ZT_LOCK_TREE = $null
    $env:ZT_LOCK_AFTER_TOOL = $null
    $env:ZT_LOCK_FLAG = $null
    Assert ($racedExit -ne 0) "the mid-run locked run stops (exit $racedExit): $($raced -join ' | ')"
    Assert (@($raced | Where-Object { $_ -match '^0 ok MidRun2023/\d+\|a$' }).Count -eq 1) "the call the owner's lock interrupted was already committed: $($raced -join ' | ')"
    Assert (@($raced | Where-Object { $_ -match '^1 error card_locked' }).Count -eq 1) "the run's next write meets the real refusal: $($raced -join ' | ')"
    Assert (@($raced | Where-Object { $_ -match '^card_locked .*"tree":"MidRun2023"' }).Count -eq 1) 'the refusal still carries its structured JSON after the owner lock'
    Assert (@($raced | Where-Object { $_ -match '^resume: -From 1 ' }).Count -eq 1) 'the run stops at that one call, never retrying it as a fault'
    Assert (@(Get-Content -LiteralPath $raceLog -Encoding utf8).Count -eq 1) 'the committed receipt stays in the run log while the refused call wrote nothing'
    $raceTree = (Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = 'MidRun2023' } })).cards
    Assert (@($raceTree | Where-Object { $_.title -eq '锁定之前写入的卡片' }).Count -eq 1 -and @($raceTree | Where-Object { $_.title -eq '锁定之后写入的卡片' }).Count -eq 0) 'the archive holds the earlier Card and none of the refused one'

    # An atomic identified chunk that the lock refuses rolls back as a whole, and its typed refusal
    # survives the helper boundary (R338): earlier committed batches are not undone.
    $identified = Join-Path $scratch 'identified.json'
    [IO.File]::WriteAllText($identified, (@(@{ tool = 'create_successor'; arguments = @{ at = 'MidRun2023/1'; kind = 'side'; title = '整批回滚的卡片'; body = '这一批整体回滚。' } }) | ConvertTo-Json -Depth 8 -AsArray), $utf8)
    $env:ZT_CHECK_ZT = $seam
    $rolled = Invoke-Tool @('call','-Open',$identified, '-OperationPrefix', 'run/race/1/step')
    $env:ZT_CHECK_ZT = $null
    Assert ($rolled.code -ne 0) "a refused identified chunk stops the run (exit $($rolled.code)): $($rolled.text)"
    Assert ($rolled.text.Contains('card_locked') -and $rolled.text.Contains('MidRun2023')) "the refused chunk keeps the typed refusal: $($rolled.text)"
    $rolledTree = (Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = 'MidRun2023' } })).cards
    Assert (@($rolledTree | Where-Object { $_.title -eq '整批回滚的卡片' }).Count -eq 0) 'the refused chunk wrote no Card at all'

    # --- an unrelated locked tree blocks nothing (R328: actual destinations only) ----------------------
    $null = New-Book 'Open2024' (Join-Path $scratch 'fourth-book')
    $fourth = Invoke-Script 'prepare-book-digest.ps1' @('-Source', $source, '-Out', (Join-Path $scratch 'fourth-book-again'),
        '-CitationKey', 'Open2024', '-BibTeX', '@book{Open2024, title={第四本书}}', '-BookTitle', '第四本书',
        '-MinUnitChars', '20', '-MaxUnits', '1', '-NoFigures', '-Apply')
    Assert ($fourth.text.Contains('Literature tree Open2024 already exists')) "an unlocked book is judged on its own destination while three trees are locked: $($fourth.text)"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
