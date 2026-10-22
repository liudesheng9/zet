#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\hub-lock.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# Tree Lock S16 (#139): a hub or register run reads a Locked tree like any other Card, may name its
# Cards as members and Link targets, and writes only at unlocked destinations. A Locked destination
# is reported as blocked with the archive's typed card_locked refusal instead of a partial hub; a
# pinned, unlocked Hub stays editable (R336) while its address is frozen; a register publication
# stops before its first write when the 索引 tree is Locked. A scratch archive with its own service;
# every Lock is a real owner Lock taken through the GUI API. The live archive is never touched.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "hub-lock-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
$archive = Join-Path $scratch 'archive'
$work = Join-Path $scratch 'hub'
New-Item -ItemType Directory -Force $archive, $work, (Join-Path $work 'out') | Out-Null
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
$dir = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $dir 'zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Invoke-Tool([string[]]$arguments) {
    $lines = @(& $Digest @arguments 2>&1 | ForEach-Object { "$_" })
    $parsed = $null
    try { $parsed = [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable } catch { }
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1]; parsed = $parsed; text = ($lines -join ' ') }
}
function Refusals($run) { @($run.lines | Where-Object { $_ -match '^\s*\{' } | ForEach-Object { try { $_ | ConvertFrom-Json } catch { } }) }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 8 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Get-Card($at) { try { @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $at } }))[0] } catch { $null } }
function Get-Tree($tree) { @(Invoke-Zt @(@{ tool = 'get_tree'; arguments = @{ tree = $tree } }))[0] }
function Save-Json($path, $value) { New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null; [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 10), $utf8) }
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
function Lock-Tree([string]$root, [string]$note = 'reviewed') {
    $gui = Get-Gui
    $plan = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note }
    if (-not $plan.plan.plan_hash) { throw "the lock plan carries no plan_hash: $($plan | ConvertTo-Json -Depth 8 -Compress)" }
    $done = Invoke-Gui $gui '/api/lock' @{ root = $root; note = $note; confirm = 'lock'; plan_hash = $plan.plan.plan_hash }
    if (-not $done.applied) { throw "the owner lock was not applied: $($done | ConvertTo-Json -Depth 8 -Compress)" }
    $done.lock
}
# A Topic of n Cards: a direct chain, each Card a claim.
function New-Topic([string]$title, [int]$count) {
    $root = (Write-Calls @(@{ tool = 'create_topic'; arguments = @{ title = $title; body = '本主题追问：这条主张怎样成立。' } }))[0] -replace '/0$', ''
    $calls = @(@{ tool = 'create_successor'; arguments = @{ at = "$root/0"; title = "$title 的第 1 条主张"; body = '第一条主张的正文。' } })
    foreach ($n in 2..$count) {
        $calls += @{ tool = 'create_successor'; arguments = @{ at = "@@$($n - 2)"; title = "$title 的第 $n 条主张"; body = "第 $n 条主张的正文。" } }
    }
    $cards = Write-Calls $calls
    [pscustomobject]@{ root = $root; cards = $cards }
}

try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $open = New-Topic '开放的主题' 8
    $locked = New-Topic '锁定的主题' 6
    # The locked tree's first Card links an open Card, so the open hub may name it as a neighbour.
    $null = Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $locked.cards[0]; body = "依赖 [[$($open.cards[0])]]：这条链接指向开放的主题。`n`n来源：[[$($open.cards[1])]]" } })
    $lock = Lock-Tree "$($locked.root)/0" 'the owner reviews this tree'
    Assert ($lock.lock_id -gt 0) "the owner locks a whole Topic tree (lock $($lock.lock_id))"

    # --- the survey reads the locked tree and marks it (R331) ------------------------------------
    $survey = Invoke-Tool @('hub', '-Step', 'survey', '-Work', $work)
    Assert ($survey.code -eq 0 -and $survey.parsed.locked -eq 1) "survey: the locked Topic is counted ($($survey.parsed.locked))"
    $surveyJson = Get-Content -Raw (Join-Path $work 'survey.json') | ConvertFrom-Json -AsHashtable
    Assert ($surveyJson.trees[$locked.root].lock.lock_id -eq $lock.lock_id) 'survey: the locked Topic carries its Lock facts'
    Assert ((Get-Content -Raw (Join-Path $work 'survey.md')) -match '已锁定') 'survey: survey.md marks the locked tree as read-only'

    # --- the plan allocates no hub job for a locked destination (R331) ---------------------------
    $limits = '{"topic_min":5,"line_min":5,"max_lines":20,"neighbour_links":3,"per_job":4,"field_min":2}'
    $planLocked = Invoke-Tool @('hub', '-Step', 'plan', '-Work', $work, '-Limits', $limits, '-Trees', $locked.root)
    Assert ($planLocked.parsed.tree_count -eq 0 -and @($planLocked.parsed.locked) -contains $locked.root) "plan: no job is allocated for the locked tree and it is named ($($planLocked.parsed.locked -join ', '))"
    $planOpen = Invoke-Tool @('hub', '-Step', 'plan', '-Work', $work, '-Limits', $limits, '-Trees', $open.root)
    Assert ($planOpen.parsed.tree_count -eq 1 -and @($planOpen.parsed.trees) -contains $open.root) 'plan: the unlocked tree still gets its hub job'

    # --- a locked Card is a valid member and Link target of an unlocked hub (R331) ---------------
    $out = Join-Path $work 'out'
    $openPlan = [ordered]@{
        tree = $open.root
        topic = [ordered]@{
            title = '开放的主题的入口'
            scope = "主题 $($open.root) 的入口。"
            groups = @([ordered]@{ name = '主张'; items = @(
                        [ordered]@{ ref = $open.cards[0]; reason = '这条主张先说明机制。' },
                        [ordered]@{ ref = $open.cards[1]; reason = '这条主张再说明结果。' }) })
            neighbours = @([ordered]@{ kind = '对照'; ref = $locked.cards[0]; reason = '锁定的树讨论同一机制，只读引用。' })
        }
        lines = @()
    }
    Save-Json (Join-Path $out "hub-$($open.root).json") $openPlan
    $pre = Invoke-Tool @('hub', '-Step', 'precheck', '-Work', $work, '-File', (Join-Path $out "hub-$($open.root).json"))
    Assert ($pre.code -eq 0 -and $pre.last -eq 'faults: 0') "precheck: a locked Card is an accepted member and neighbour ($($pre.last))"
    # A hub file for the locked tree (a plan made while it was free) must be left alone.
    Save-Json (Join-Path $out "hub-$($locked.root).json") ([ordered]@{
            tree = $locked.root
            topic = [ordered]@{ title = '锁定的主题的入口'; scope = "主题 $($locked.root) 的入口。"; groups = @([ordered]@{ name = '主张'; items = @([ordered]@{ ref = $locked.cards[0]; reason = '锁定树里的第一张卡。' }, [ordered]@{ ref = $locked.cards[1]; reason = '锁定树里的第二张卡。' }) }); neighbours = @() }
            lines = @()
        })
    $applied = Invoke-Tool @('hub', '-Step', 'apply', '-Work', $work)
    Assert ($applied.code -eq 0 -and @($applied.parsed.blocked | Where-Object { $_.tree -eq $locked.root }).Count -eq 1) "apply: the locked tree is reported as blocked ($($applied.parsed.blocked | ConvertTo-Json -Compress))"
    $blockedEntry = @($applied.parsed.blocked | Where-Object { $_.tree -eq $locked.root })[0]
    Assert ($blockedEntry.refusal.error -eq 'card_locked' -and $blockedEntry.refusal.tree -eq "$($locked.root)/0" -and $blockedEntry.refusal.reason -eq 'create-inside') 'apply: the blocked tree carries the typed card_locked refusal'
    Assert ($applied.parsed.complete -eq $false) 'apply: a blocked plan is not reported complete'
    $openTree = Get-Tree $open.root
    $openHub = @($openTree.cards | Where-Object { $_.type -eq 'hub' -and $_.level -eq 'topic' })[0]
    $openHubCard = Get-Card $openHub.address
    Assert ($openHub -and $openHubCard.body -match [regex]::Escape("[[$($locked.cards[0])]]")) "apply: the unlocked hub names the locked Card as a neighbour ($($openHubCard.body -replace "`n", ' / '))"
    $lockedTree = Get-Tree $locked.root
    Assert (@($lockedTree.cards | Where-Object { $_.type -eq 'hub' }).Count -eq 0) 'apply: no hub was written into the locked tree'
    $hubReport = @(Invoke-Zt @(@{ tool = 'get_hub'; arguments = @{ at = $openHub.address } }))[0]
    Assert (@($hubReport.faults | Where-Object { $_.fault -eq 'broken' }).Count -eq 0) "get_hub: a locked member is not a broken or illegal member ($($hubReport.faults | ConvertTo-Json -Compress))"

    # --- R336: a pinned, unlocked Hub stays editable; only its address is frozen -----------------
    $pinner = New-Topic '引用枢纽的主题' 3
    $null = Write-Calls @(@{ tool = 'edit_card'; arguments = @{ at = $pinner.cards[0]; body = "对照 [[$($openHub.address)]]：这张卡链接开放的枢纽，把它钉住。`n`n来源：[[$($open.cards[0])]]" } })
    $pinLock = Lock-Tree "$($pinner.root)/0" 'pins the open hub'
    Assert ((Get-Card $openHub.address).pinned_by -contains "$($pinner.root)/0") 'R336: the locked tree pins the unlocked Hub Card'
    # The compiler still updates the pinned hub: its body and title, never its address.
    $openPlan.topic.title = '开放的主题的入口（第二稿）'
    $openPlan.topic.groups[0].items[1].reason = '这条主张改写后说明结果。'
    Save-Json (Join-Path $out "hub-$($open.root).json") $openPlan
    $again = Invoke-Tool @('hub', '-Step', 'apply', '-Work', $work)
    $pinnedNow = Get-Card $openHub.address
    Assert ($pinnedNow.title -eq '开放的主题的入口（第二稿）' -and $pinnedNow.body -match '改写后说明结果') 'R336: a pinned, unlocked Hub Card is still edited by the hub run'
    Assert ($again.parsed.complete -eq $false) 'R336: the locked tree still blocks only its own destination'
    $moved = $null
    try {
        $moved = Invoke-Zt @(@{ tool = 'move_card'; arguments = @{ at = $openHub.address; before = $open.cards[2]; confirm = 'move' } })
    } catch { $moved = "$_" }
    Assert ("$moved" -match 'card_locked' -and "$moved" -match 'pinned-move') "R336: the pinned Hub cannot move ($moved)"

    # --- the register: a locked target stays readable; a locked 索引 tree stops the publication --
    $regWork = Join-Path $scratch 'register'
    New-Item -ItemType Directory -Force (Join-Path $regWork 'out') | Out-Null
    $book = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = '@book{LockReg2026, title={锁下的索引}, author={Owner, A.}, year={2026}}'; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'LockReg2026'; kind = 'direct'; title = '第 1 组：均衡'; body = '§1 模型与术语' } }
    )
    $tableBody = @'
本卡是《锁下的索引》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 剩余控制权 | 控制权 | 契约没有写明的权利 | §1.1 |
| 所有权 | 产权 | 剩余控制权的来源 | §1.2 |
'@
    $null = Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = $book[1]; kind = 'side'; title = '术语表：锁下的索引'; body = $tableBody } })
    $entryCards = Write-Calls @(
        @{ tool = 'create_topic'; arguments = @{ title = '剩余控制权的来源'; body = '剩余控制权从哪里来？' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '剩余控制权来自所有权'; body = '所有者拥有契约没写明的权利。' } }
    )
    $regPlan = Invoke-Tool @('register', '-Step', 'plan', '-Work', $regWork)
    Assert ($regPlan.code -eq 0 -and $regPlan.parsed.ok -eq $true) "register plan: a plan is staged ($($regPlan.parsed.terms) terms)"
    $regApplied = Invoke-Tool @('register', '-Step', 'apply', '-Work', $regWork)
    Assert ($regApplied.code -eq 0 -and $regApplied.parsed.verified -eq $true) 'register apply: the register is published'
    $register = $regApplied.parsed.register
    $indexRoot = ($register -split '/')[0] + '/0'
    # A register that names a locked Card reads normally: the locked Card is a read-only target.
    $targetTree = ($entryCards[0] -split '/')[0]
    $null = Lock-Tree "$targetTree/0" 'the target is reviewed'
    $hubAfter = @(Invoke-Zt @(@{ tool = 'get_hub'; arguments = @{ at = $register } }))[0]
    Assert (@($hubAfter.faults | Where-Object { $_.fault -eq 'broken' }).Count -eq 0) 'register: a locked target is a valid read-only entry'
    $check = Invoke-Tool @('register', '-Step', 'check', '-Work', $regWork)
    Assert ($check.code -eq 0 -and $check.parsed.complete -eq $true) 'register check: the published register still matches its plan'
    # The 索引 tree itself is locked: the next publication stops before its first write.
    $bodyBefore = (Get-Card $register).text
    $titlesBefore = @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } }))[0].count
    $indexLock = Lock-Tree $indexRoot 'the register is reviewed'
    $stopped = Invoke-Tool @('register', '-Step', 'apply', '-Work', $regWork)
    Assert ($stopped.code -eq 2) "register apply: a locked 索引 tree stops the publication with the Lock exit code ($($stopped.code))"
    $refusal = @(Refusals $stopped | Where-Object { $_.error -eq 'card_locked' })[0]
    Assert ($null -ne $refusal -and $refusal.tree -eq $indexRoot -and $refusal.reason -eq 'edit') "register apply: the stop answers the typed card_locked JSON ($($stopped.text))"
    Assert ((Get-Card $register).text -eq $bodyBefore) 'register apply: the refused publication changed no Card'
    $titlesAfter = @(Invoke-Zt @(@{ tool = 'export_titles'; arguments = @{ workspace = 'agent' } }))[0].count
    Assert ($titlesAfter -eq $titlesBefore) 'register apply: the refused publication created no Card'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
