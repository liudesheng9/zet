#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\prepare.tests.ps1 [-Zt <zt.exe>]
# Drives prepare-book-digest.ps1, snapshot-topics.ps1, and prepare-link-pass.ps1 (forwarders to
# `zt-digest prepare`, `snapshot-topics`, and `prepare-link`) on a scratch archive (ZT_CONFIG_DIR, its
# own service). A small bilingual book with no Scholar Archive file (-NoFigures): the Chinese text,
# the units and groups, the group files with their helper and SELF-CHECK lines, the event candidates,
# the shared prefix, and with -Apply the Literature tree (a second -Apply is refused). Then the
# snapshot of every tree, and the link pass: ownership from owned-gNN.txt, from an anchor's Side
# successor, and from a `§` reference; the allow files; linked-gNN.txt takes a group off the pending
# list. The live archive is not touched. -Zt runs a zt build that is not installed yet.
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "prepare-test-$PID"
$scratch = [IO.Path]::GetFullPath($scratch)
$testTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($testTempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch path escaped the intended test temp directory' }
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
$here = Join-Path $PSScriptRoot '..' | Resolve-Path | ForEach-Object Path
. (Join-Path $here 'zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Invoke-Script($name, [string[]]$arguments) {
    $lines = @(pwsh -NoProfile -File (Join-Path $here $name) @arguments 2>&1 | ForEach-Object { "$_" })
    [pscustomobject]@{ code = $LASTEXITCODE; lines = $lines; last = @($lines)[-1] }
}

$book = @'
# Preface

Some preface.

# 导论

## Intro.1 Why firms

Firms exist because markets fail.

企业存在是因为市场失灵。1937 年科斯提出了这个问题。

$$x = y$$

# 第一章 契约

## 1.1 不完全契约

Contracts are incomplete.

契约不完全，Hart (1995) 讨论了剩余控制权。通用汽车在 1926 年收购了费雪车身公司，1990 年代的变化不算事件。

## 1.2 敲竹杠

Hold-up.

敲竹杠问题使专用性投资不足。

## 习题

练习一。

# 参考

文献。
'@

try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $source = Join-Path $scratch 'book.md'
    [IO.File]::WriteAllText($source, $book, $utf8)
    $staging = Join-Path $scratch 'staging'
    $prepare = @('-Source', $source, '-Out', $staging, '-CitationKey', 'Test2021', '-BibTeX', '@book{Test2021, title={Firms and Contracts}}',
        '-BookTitle', '企业与契约', '-SkipHeadings', '习题', '-MinUnitChars', '20', '-MaxUnits', '1', '-NoFigures', '-Apply')

    # --- prepare --------------------------------------------------------------------------------------
    $run = Invoke-Script 'prepare-book-digest.ps1' $prepare
    Assert ($run.code -eq 0) "prepare runs: $($run.lines -join ' | ')"
    $manifest = $run.last | ConvertFrom-Json
    $saved = Get-Content -LiteralPath (Join-Path $staging 'manifest.json') -Raw -Encoding utf8 | ConvertFrom-Json
    Assert ($saved.citation_key -eq 'Test2021' -and $saved.groups.Count -eq 3) 'manifest.json names the book and its three groups'
    Assert (($manifest.groups | ForEach-Object { $_.headings -join '+' }) -join ' / ' -eq '导论+Intro.1 Why firms / 第一章 契约+1.1 不完全契约 / 1.2 敲竹杠') "a short unit goes into the next one: $(($manifest.groups | ForEach-Object { $_.headings -join '+' }) -join ' / ')"
    Assert (@($manifest.created).Count -eq 4 -and $manifest.created[0] -eq '0 ok Test2021') "-Apply creates the Literature Card and three anchors: $($manifest.created -join ', ')"
    $text = Get-Content -LiteralPath (Join-Path $staging 'book.zh.md') -Raw -Encoding utf8
    Assert ($text -match '企业存在' -and $text -notmatch 'Firms exist' -and $text -match '\$\$x = y\$\$' -and $text -notmatch '练习一') 'book.zh.md keeps the Chinese and formulas, drops the English and the skipped section'
    $g2 = Get-Content -LiteralPath (Join-Path $staging 'groups\g02.md') -Raw -Encoding utf8
    $owned = Join-Path $manifest.staging 'owned-g02.txt'
    Assert ($g2.Contains("- 写入命令：``... | zt-digest call - -Log `"$owned`" -SrcLog")) 'the group file names the helper as zt-digest call'
    Assert ($g2.Contains("- SELF-CHECK: ``zt-check --cards `"Test2021/2`" --log `"$owned`" --book Test2021``")) 'the group file keeps its zt-check SELF-CHECK line'
    Assert ($g2 -match 'sections/s02\.md:\d+` 契约不完全') 'the group file lists its event candidates'
    $events = @(Get-Content -LiteralPath (Join-Path $staging 'events.json') -Raw -Encoding utf8 | ConvertFrom-Json)
    Assert ($events.Count -eq 2 -and $events[0].group -eq 1 -and $events[1].src -match '^sections/s02\.md:') "events.json lists the lines that name a year: $($events.src -join ', ')"
    $prefix = Get-Content -LiteralPath $manifest.shared_prefix -Raw -Encoding utf8
    Assert ($prefix.Contains('[[Test2021]]') -and $prefix.Contains('《企业与契约》') -and -not $prefix.Contains('{{')) 'the shared prefix has its placeholders filled'
    $anchor = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = 'Test2021/3' } })
    Assert ($anchor.title -eq '第 3 组：1.2 敲竹杠') "the anchor Card names its group: $($anchor.title)"
    $again = Invoke-Script 'prepare-book-digest.ps1' $prepare
    Assert ($again.code -ne 0 -and ($again.lines -join ' ') -match 'Literature tree Test2021 already exists') 'a second -Apply is refused'

    # --- the digest writes some Cards -----------------------------------------------------------------
    $null = Invoke-Zt @(
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2021/1'; kind = 'side'; title = '市场失灵使企业出现'; body = "企业替代市场。`n`n来源：[[Test2021]] §Intro.1" } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2021/2'; kind = 'side'; title = '契约不完全留下剩余控制权'; body = "剩余控制权归所有者。`n`n来源：[[Test2021]] §1.1" } },
        @{ tool = 'create_topic'; arguments = @{ title = '企业边界'; body = '追问企业边界。' } },
        @{ tool = 'create_successor'; arguments = @{ at = '@@2'; title = '敲竹杠使专用性投资不足'; body = "§1.2 敲竹杠让投资方担心被压价。`n`n来源：[[Test2021]] §1.2" } }
    )
    [IO.File]::WriteAllText((Join-Path $staging 'owned-g01.txt'), "Test2021/1|a`n", $utf8)

    # --- snapshot-topics ------------------------------------------------------------------------------
    $snapshotFile = Join-Path $staging 'snapshot-g01.md'
    $snapshotTrace = Join-Path $scratch 'snapshot-tools.jsonl'
    $savedCheckZt = $env:ZT_CHECK_ZT
    $savedTrace = $env:ZT_TRACE_TOOLS
    $savedFailTree = $env:ZT_FAIL_TREE
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS = $snapshotTrace
    $env:ZT_FAIL_TREE = 'Test2021'
    try {
    $snap = Invoke-Script 'snapshot-topics.ps1' @('-Out', $snapshotFile)
    Assert ($snap.code -eq 0 -and $snap.last -eq "8 cards -> $snapshotFile") "snapshot-topics counts every Card: $($snap.lines -join ' | ')"
    $snapText = Get-Content -LiteralPath $snapshotFile -Raw -Encoding utf8
    Assert ($snapText.Contains("- ``Test2021`` ") -and $snapText.Contains("  - ``Test2021/2|a`` 契约不完全留下剩余控制权") -and $snapText.Contains('## 人类工作区（只读，可链接）')) 'the snapshot lists each Card under its Workspace'
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = 'Test2021/2|a'; title = '契约不完全使未来条款留下空缺' } })
    $snapshot2 = Join-Path $staging 'snapshot-g02.md'
    $snap2 = Invoke-Script 'snapshot-topics.ps1' @('-Out', $snapshot2)
    $snapText2 = Get-Content -LiteralPath $snapshot2 -Raw -Encoding utf8
    Assert ($snap2.code -eq 0 -and $snapText2.Contains('契约不完全使未来条款留下空缺') -and $snapText.Contains('契约不完全留下剩余控制权')) 'each group gets its own fresh title export after another group changes a Card'
    $trace = @(Get-Content -LiteralPath $snapshotTrace -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($trace | Where-Object tool -eq 'export_titles').Count -eq 2 -and @($trace | Where-Object tool -in @('get_card', 'get_tree', 'list_trees')).Count -eq 0) 'snapshot uses exactly one export per group and no per-tree or per-Card reads'
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = 'Test2021/2|a'; title = '契约不完全留下剩余控制权' } })
    } finally {
        $env:ZT_CHECK_ZT = $savedCheckZt
        $env:ZT_TRACE_TOOLS = $savedTrace
        $env:ZT_FAIL_TREE = $savedFailTree
    }

    # --- prepare-link-pass ----------------------------------------------------------------------------
    $null = Invoke-Zt @(@{ tool = 'edit_card'; arguments = @{ at = 'Test2021/2|a'; body = "剩余控制权归所有者。`n`n参照 [[0/1]]，敲竹杠使专用性投资不足，两个问题都依赖未来履约。`n`n来源：[[Test2021]] §1.1" } })
    $linkTrace = Join-Path $scratch 'link-tools.jsonl'
    $savedCheckZt = $env:ZT_CHECK_ZT
    $savedTrace = $env:ZT_TRACE_TOOLS
    $savedFailTree = $env:ZT_FAIL_TREE
    $savedFailCard = $env:ZT_FAIL_CARD
    $savedFailFlag = $env:ZT_FAIL_FLAG
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS = $linkTrace
    $env:ZT_FAIL_TREE = '0'
    $env:ZT_FAIL_CARD = 'Test2021/1|a'
    $env:ZT_FAIL_FLAG = Join-Path $scratch 'bulk-only.flag'
    [IO.File]::WriteAllText($env:ZT_FAIL_FLAG, '', $utf8)
    try { $link = Invoke-Script 'prepare-link-pass.ps1' @('-Staging', $staging) }
    finally {
        $env:ZT_CHECK_ZT = $savedCheckZt
        $env:ZT_TRACE_TOOLS = $savedTrace
        $env:ZT_FAIL_TREE = $savedFailTree
        $env:ZT_FAIL_CARD = $savedFailCard
        $env:ZT_FAIL_FLAG = $savedFailFlag
    }
    Assert ($link.code -eq 0) "prepare-link-pass runs: $($link.lines -join ' | ')"
    $summary = $link.last | ConvertFrom-Json
    $trace = @(Get-Content -LiteralPath $linkTrace -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($trace | Where-Object tool -eq 'export_titles').Count -eq 1 -and @($trace | Where-Object tool -eq 'get_cards').Count -eq 1 -and @($trace | Where-Object tool -in @('get_card', 'get_tree', 'list_trees')).Count -eq 0 -and @($trace.process_id | Sort-Object -Unique).Count -eq 1) 'prepare-link uses one title export and one scoped bulk read through one MCP process'
    Assert (($summary.pending -join ',') -eq '1,2,3' -and ($summary.groups.owned -join ',') -eq '2,2,2' -and $summary.unowned -eq 1) "each group owns its anchor and one Card: $($link.last)"
    $allow3 = Get-Content -LiteralPath (Join-Path $staging 'allow-g03.txt') -Encoding utf8
    Assert ($allow3.Count -eq 2 -and $allow3[0] -eq '0/1' -and $allow3[1] -eq 'Test2021/3') "a section reference gives a Topic Card to its group: $($allow3 -join ',')"
    $l2 = Get-Content -LiteralPath (Join-Path $staging 'link-g02.md') -Raw -Encoding utf8
    $allow2 = Join-Path $staging 'allow-g02.txt'
    Assert ($l2.Contains("- 写入命令：``... | zt-digest call - -Allow `"$allow2`"``") -and $l2.Contains("- SELF-CHECK: ``zt-check --log `"$allow2`" --cards `"`" --book Test2021 --rules L1,L2,R2,R3,R4,R5,R7,R9``")) 'the link file names the helper and keeps its SELF-CHECK line'
    Assert ($l2.Contains("### ``Test2021/2|a`` 契约不完全留下剩余控制权") -and $l2.Contains('剩余控制权归所有者。')) 'the link file shows each owned Card with its body'
    $candidateLines = @($l2 -split "`n" | Where-Object { $_ -match '^> 候选目标' }) -join "`n"
    Assert (-not $candidateLines.Contains('`0/1`')) 'bulk preparation preserves the exclusion of already-linked candidate targets'
    $all = Get-Content -LiteralPath $summary.cards_all -Raw -Encoding utf8
    Assert ($all.Contains("  - ``0/1`` 敲竹杠使专用性投资不足")) 'cards-all.md lists every Card'
    $savedCheckZt = $env:ZT_CHECK_ZT
    $savedBulkLimit = $env:ZT_BULK_LIMIT
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_BULK_LIMIT = '3'
    try { $limited = Invoke-Script 'prepare-link-pass.ps1' @('-Staging', $staging) }
    finally { $env:ZT_CHECK_ZT = $savedCheckZt; $env:ZT_BULK_LIMIT = $savedBulkLimit }
    Assert ($limited.code -eq 0 -and (($limited.last | ConvertFrom-Json).groups.owned -join ',') -eq '2,2,2') 'a bulk reply limit reduces chunks without losing any required Cards'
    $savedCheckZt = $env:ZT_CHECK_ZT
    $savedMissing = $env:ZT_BULK_MISSING
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_BULK_MISSING = 'Test2021/1|a'
    try { $missing = Invoke-Script 'prepare-link-pass.ps1' @('-Staging', $staging) }
    finally { $env:ZT_CHECK_ZT = $savedCheckZt; $env:ZT_BULK_MISSING = $savedMissing }
    Assert ($missing.code -ne 0 -and ($missing.lines -join ' ').Contains('incomplete Card read') -and ($missing.lines -join ' ').Contains('Test2021/1|a')) 'a missing required bulk read fails explicitly instead of producing an empty successful dataset'
    [IO.File]::WriteAllText((Join-Path $staging 'linked-g01.txt'), "done`n", $utf8)
    $rerun = (Invoke-Script 'prepare-link-pass.ps1' @('-Staging', $staging)).last | ConvertFrom-Json
    Assert (($rerun.pending -join ',') -eq '2,3' -and $rerun.groups[0].done) 'linked-gNN.txt takes a group off the pending list'
    # Group 2 has no owned log, so the structure/index inference guard runs.
    $targets = @('Test2021','Test2021/1','Test2021/2','Test2021/3','Test2021/1|a','Test2021/2|a','0/0','0/1')
    $indexBody = "来源：[[Test2021]] §1.1`n`n" + (($targets | ForEach-Object { "[[$_]]" }) -join "`n")
    $null = Invoke-Zt @(@{tool='create_successor';arguments=@{at='Test2021/2';kind='side';title='结构索引';body=$indexBody}})
    $structureRun = Invoke-Script 'prepare-link-pass.ps1' @('-Staging', $staging)
    $structureAllow = @(Get-Content -LiteralPath (Join-Path $staging 'allow-g02.txt') -Encoding utf8)
    Assert ($structureRun.code -eq 0 -and 'Test2021/2|b' -notin $structureAllow) 'bulk ownership inference preserves the eight-distinct-outbound structure guard with repeated targets'
    # export_titles has ZET numeric order; the digest snapshot retains the former
    # list_trees/get_tree rendering order, including multi-digit Topic ids.
    $null = Invoke-Zt @(1..12 | ForEach-Object { @{tool='create_topic';arguments=@{title="Ordering $_"}} })
    $orderedSnapshot = Join-Path $staging 'snapshot-ordering.md'
    $ordered = Invoke-Script 'snapshot-topics.ps1' @('-Out', $orderedSnapshot)
    $orderedText = Get-Content -LiteralPath $orderedSnapshot -Raw -Encoding utf8
    Assert ($ordered.code -eq 0 -and $orderedText.IndexOf('- `10/0`') -ge 0 -and $orderedText.IndexOf('- `10/0`') -lt $orderedText.IndexOf('- `2/0`')) 'the fresh title export preserves the former multi-digit Topic tree rendering order'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { Write-Output "$failed failed"; exit 1 }
Write-Output 'all passed'
