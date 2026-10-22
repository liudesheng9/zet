#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\zt-helper-gate.tests.ps1 [-Zt <zt.exe>]
# Checks the zt helper's -SrcLog and -TopicGate (docs/digest-card-layers-proposal.md, proposal 4) on
# a scratch archive with its own service: `src` never reaches zt and is logged; a Topic with the same
# or a very similar title is refused with its address; a new title passes; two helpers that create
# the same Topic at the same time make it once.
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = Join-Path ([IO.Path]::GetTempPath()) "zt-helper-gate-test-$PID"
$archive = Join-Path $scratch 'archive'
New-Item -ItemType Directory -Force $archive | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$helper = Join-Path $PSScriptRoot '..\..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Helper($calls, [string[]]$more = @()) { @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $helper - -Open @more) }
$gate = Join-Path $scratch 'topic-gate.lock'
$srcLog = Join-Path $scratch 'src-g01.txt'
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $out = Helper @(
        @{ tool = 'create_topic'; arguments = @{ title = '剩余控制权'; body = '本主题追问：谁决定契约没写的事。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '所有者决定空白处的用法'; body = '正文。'; src = 'sections/s02.md:10-20' } }
    ) @('-SrcLog', $srcLog, '-TopicGate', $gate)
    Assert ($LASTEXITCODE -eq 0 -and @($out | Where-Object { $_ -match '^\d+ ok ' }).Count -eq 2) "a new Topic and a Card with src are made ($($out -join ' | '))"
    $card = ($out[1] -split ' ok ', 2)[1].Trim()
    $log = @(Get-Content -LiteralPath $srcLog -Encoding utf8)
    Assert ($log.Count -eq 1 -and $log[0] -eq "$card`tsections/s02.md:10-20") "the src log holds the Card and its source ($($log -join ' | '))"
    $read = Helper @(@{ tool = 'get_card'; arguments = @{ at = $card } })
    Assert (($read -join ' ') -notmatch 'src') 'src never reaches the Card'

    $same = Helper @(@{ tool = 'create_topic'; arguments = @{ title = '剩余 控制权！'; body = '重复。' } }) @('-TopicGate', $gate)
    Assert ($LASTEXITCODE -ne 0 -and ($same -join ' ') -match 'topic exists: `\d+/0` 剩余控制权') "the same title is refused with the Topic's address ($($same -join ' | '))"
    Assert (($same -join ' ') -match 'resume: -From 0') 'the refusal gives a resume line'
    $near = Helper @(@{ tool = 'create_topic'; arguments = @{ title = '剩余控制权的'; body = '近似。' } }) @('-TopicGate', $gate)
    Assert ($LASTEXITCODE -ne 0 -and ($near -join ' ') -match 'topic exists') 'a very similar title is refused'
    $other = Helper @(@{ tool = 'create_topic'; arguments = @{ title = '敲竹杠'; body = '本主题追问：专用投资后谁占便宜。' } }) @('-TopicGate', $gate)
    Assert ($LASTEXITCODE -eq 0) 'a different title passes'
    $free = Helper @(@{ tool = 'create_topic'; arguments = @{ title = '剩余控制权'; body = '没有闸门时照常创建。' } })
    Assert ($LASTEXITCODE -eq 0) 'without -TopicGate the helper does not check'

    # docs/literature-only-digest-design.md: without -Open the helper runs in the Literature scope, as
    # every digest does, and refuses a Topic, a Topic-tree write and a Link to a Topic-tree Card.
    $topicRoot = ($out[0] -split ' ok ', 2)[1].Trim()
    $closed = @((@(@{ tool = 'create_topic'; arguments = @{ title = '闭合范围'; body = '不应创建。' } }) | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $helper - 2>&1 | ForEach-Object { [string]$_ })
    Assert ($LASTEXITCODE -ne 0 -and ($closed -join ' ') -match 'literature_only: create_topic') "without -Open the helper refuses create_topic ($($closed -join ' | '))"
    $inTopic = @((@(@{ tool = 'create_successor'; arguments = @{ at = $card; title = '主题树里的卡'; body = '不应创建。' } }) | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $helper - 2>&1 | ForEach-Object { [string]$_ })
    Assert ($LASTEXITCODE -ne 0 -and ($inTopic -join ' ') -match "literature_only: create_successor: .*``$([regex]::Escape($card))`` is a Topic-tree Card") "without -Open the helper refuses a Card in a Topic tree ($($inTopic -join ' | '))"
    $lit = @((@(@{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Scope2026, title = {Scope}}'; body = "对照 [[$topicRoot]]。" } }) | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $helper - 2>&1 | ForEach-Object { [string]$_ })
    Assert ($LASTEXITCODE -ne 0 -and ($lit -join ' ') -match 'links Topic-tree Cards') "without -Open the helper refuses a Link to a Topic-tree Card ($($lit -join ' | '))"
    $book = @((@(@{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Scope2026, title = {Scope}}'; body = '文献卡。' } }, @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '文献树里的卡'; body = '依赖 [[@@0]]。' } }) | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $helper - 2>&1 | ForEach-Object { [string]$_ })
    Assert ($LASTEXITCODE -eq 0 -and @($book | Where-Object { $_ -match '^\d+ ok ' }).Count -eq 2) "without -Open the helper still writes a Literature tree ($($book -join ' | '))"

    # Two helpers at once: the lock lets one through; the other sees its Topic and is refused.
    $script = { param($helper, $gate) (@(@{ tool = 'create_topic'; arguments = @{ title = '纵向一体化'; body = '并发。' } }) | ConvertTo-Json -Depth 4 -AsArray) | pwsh -NoProfile -File $helper - -Open -TopicGate $gate }
    $jobs = @(1, 2) | ForEach-Object { Start-ThreadJob -ScriptBlock $script -ArgumentList $helper, $gate }
    $results = @($jobs | Wait-Job | Receive-Job)
    $made = @($results | Where-Object { $_ -match '^0 ok ' }).Count
    $refused = @($results | Where-Object { $_ -match 'topic exists' }).Count
    Assert ($made -eq 1 -and $refused -eq 1) "two helpers at once make the Topic once (made $made, refused $refused)"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
