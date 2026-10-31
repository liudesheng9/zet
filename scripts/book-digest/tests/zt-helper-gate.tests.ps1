#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\zt-helper-gate.tests.ps1 [-Zt <zt.exe>]
# Checks the zt helper path (skills/zt/scripts/zt-mcp.ps1 -> zt-digest call / open-call) on a scratch
# archive with its own service (docs/agent-write-scope-proposal.md, #226): `src` never reaches zt and
# is logged; the server refuses an agent Topic whose title another agent Topic has, with its address;
# a new title passes; two helpers that create the same Topic at the same time make it once; `call`
# runs in the server's Literature scope and has no -Open switch; `open-call` is the open scope.
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
function Open($calls, [string[]]$more = @()) { @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $helper open-call - @more 2>&1 | ForEach-Object { [string]$_ }) }
function Literature($calls, [string[]]$more = @()) { @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $helper - @more 2>&1 | ForEach-Object { [string]$_ }) }
$srcLog = Join-Path $scratch 'src-g01.txt'
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $out = Open @(
        @{ tool = 'create_topic'; arguments = @{ title = '剩余控制权'; body = '本主题追问：谁决定契约没写的事。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '所有者决定空白处的用法'; body = '正文。'; src = 'sections/s02.md:10-20' } }
    ) @('-SrcLog', $srcLog)
    Assert ($LASTEXITCODE -eq 0 -and @($out | Where-Object { $_ -match '^\d+ ok ' }).Count -eq 2) "a new Topic and a Card with src are made ($($out -join ' | '))"
    $card = ($out[1] -split ' ok ', 2)[1].Trim()
    $log = @(Get-Content -LiteralPath $srcLog -Encoding utf8)
    Assert ($log.Count -eq 1 -and $log[0] -eq "$card`tsections/s02.md:10-20") "the src log holds the Card and its source ($($log -join ' | '))"
    $read = Open @(@{ tool = 'get_card'; arguments = @{ at = $card } })
    Assert (($read -join ' ') -notmatch 'src') 'src never reaches the Card'

    $same = Open @(@{ tool = 'create_topic'; arguments = @{ title = '剩余 控制权！'; body = '重复。' } })
    Assert ($LASTEXITCODE -ne 0 -and ($same -join ' ') -match 'topic exists: `\d+/0` 剩余控制权') "the same title is refused with the Topic's address ($($same -join ' | '))"
    Assert (@($same | Where-Object { $_ -match '^topic_exists \{' }).Count -eq 1) 'the refusal is the typed topic_exists line'
    Assert (($same -join ' ') -match 'resume: -From 0') 'the refusal gives a resume line'
    $other = Open @(@{ tool = 'create_topic'; arguments = @{ title = '敲竹杠'; body = '本主题追问：专用投资后谁占便宜。' } })
    Assert ($LASTEXITCODE -eq 0) 'a different title passes'
    $gone = Open @(@{ tool = 'get_card'; arguments = @{ at = $card } }) @('-TopicGate', (Join-Path $scratch 'gate.lock'))
    Assert ($LASTEXITCODE -ne 0 -and ($gone -join ' ') -match 'unknown parameter -TopicGate') "the client Topic gate is gone ($($gone -join ' | '))"

    # The helper `call` runs in the server's Literature scope, as every digest does.
    $topicRoot = ($out[0] -split ' ok ', 2)[1].Trim()
    $closed = Literature @(@{ tool = 'create_topic'; arguments = @{ title = '闭合范围'; body = '不应创建。' } })
    Assert ($LASTEXITCODE -ne 0 -and ($closed -join ' ') -match 'scope_refused \{.*"reason":"topic_tree"') "call refuses create_topic with the server's scope_refused ($($closed -join ' | '))"
    $inTopic = Literature @(@{ tool = 'create_successor'; arguments = @{ at = $card; title = '主题树里的卡'; body = '不应创建。' } })
    Assert ($LASTEXITCODE -ne 0 -and ($inTopic -join ' ') -match "``$([regex]::Escape($card))`` is a Topic-tree Card") "call refuses a Card in a Topic tree ($($inTopic -join ' | '))"
    $lit = Literature @(@{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Scope2026, title = {Scope}}'; body = "对照 [[$topicRoot]]。" } })
    Assert ($LASTEXITCODE -ne 0 -and ($lit -join ' ') -match '"reason":"topic_link"') "call refuses a Link to a Topic-tree Card ($($lit -join ' | '))"
    $book = Literature @(@{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Scope2026, title = {Scope}}'; body = '文献卡。' } }, @{ tool = 'create_successor'; arguments = @{ at = '@@0'; title = '文献树里的卡'; body = '依赖 [[@@0]]。' } })
    Assert ($LASTEXITCODE -eq 0 -and @($book | Where-Object { $_ -match '^\d+ ok ' }).Count -eq 2) "call still writes a Literature tree ($($book -join ' | '))"
    $switch = Literature @(@{ tool = 'get_card'; arguments = @{ at = $card } }) @('-Open')
    Assert ($LASTEXITCODE -ne 0 -and ($switch -join ' ') -match 'open-call') "call has no -Open switch and names open-call ($($switch -join ' | '))"

    # Two helpers at once: the server's edit lock lets one through; the other sees its Topic.
    $script = { param($helper) (@(@{ tool = 'create_topic'; arguments = @{ title = '纵向一体化'; body = '并发。' } }) | ConvertTo-Json -Depth 4 -AsArray) | pwsh -NoProfile -File $helper open-call - }
    $jobs = @(1, 2) | ForEach-Object { Start-ThreadJob -ScriptBlock $script -ArgumentList $helper }
    $results = @($jobs | Wait-Job | Receive-Job)
    $made = @($results | Where-Object { $_ -match '^0 ok ' }).Count
    $refused = @($results | Where-Object { $_ -match '^topic_exists \{' }).Count
    Assert ($made -eq 1 -and $refused -eq 1) "two helpers at once make the Topic once (made $made, refused $refused)"
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
