#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\standalone-and-source.tests.ps1
# #93 S02: the standalone rules R20 and R21 and the own-source rule I7 at their public seams.
# A real scratch archive, the public `zt-check --cards` contract, and the scoped audit of
# zt-digest. No provider runs; the installed binaries are the ones on PATH.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "standalone-rules-test-$PID"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, [string]$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $made = Write-Cards @(
        @{tool='create_literature';arguments=@{bibtex='@book{Key2026,title={Key},year={2026}}'}},
        @{tool='create_successor';arguments=@{at='Key2026';title='第一组';body="§1 契约`n`n§2 所有权"}},
        @{tool='create_literature';arguments=@{bibtex='@book{Other2020,title={Other},year={2020}}'}},
        @{tool='create_successor';arguments=@{at='Other2020';title='第一组';body='§1 另一本书'}},
        @{tool='create_topic';arguments=@{title='剩余控制权';body='谁在契约空白处做决定？'}}
    )
    $key, $anchor, $other, $otherAnchor, $topic = $made
    $cards = Write-Cards @(
        # R20 on an unexplained label and R21 on a bare deictic: both advice.
        @{tool='create_successor';arguments=@{at=$topic;title='剩余控制权决定契约空白处';body="式 (12) 有该项。`n`n该卡给出了结论。`n`n来源：[[$key]] §1"}},
        # A label with its content, a source locator, a name, code, a quote, and inline code.
        @{tool='create_successor';arguments=@{at='@@0';kind='side';title='加成项来自需求弹性';body="式 (12) 的加成项来自需求弹性。`n`n命题 3.G.3 说明均衡唯一。`n`n第一福利定理在这里成立。`n`n``````n式 (12) 有该项`n```````n`n> 该卡有该项`n`n``式 (12)`` 有该项。`n`n来源：[[$key]] §1"}},
        # R21 passes when the same sentence names a Link, and fails when only the sentence before does.
        @{tool='create_successor';arguments=@{at='@@0';kind='side';title='该卡说明了均衡';body="先看 [[@@0]] 的方法。该卡给出了结果。"}},
        # I7: a rests-on Link into the Card's own source tree.
        @{tool='create_successor';arguments=@{at='@@0';kind='side';title='剩余控制权提高谈判地位';body="依赖 [[$anchor]] 的设定：所有者能拒绝合作。`n`n来源：[[$key]] §1"}},
        # No I7: an illustrative Link is no dependency.
        @{tool='create_successor';arguments=@{at='@@0';kind='side';title='两期模型例示同一机制';body="例示 [[$anchor]] 的两期模型。`n`n来源：[[$key]] §1"}},
        # No I7: another book is not the Card's own source.
        @{tool='create_successor';arguments=@{at='@@0';kind='side';title='专用性投资不可转用';body="依赖 [[$otherAnchor]] 的前提：投资不可转用。`n`n来源：[[$key]] §2"}}
    )
    # I7 unknown: a rests-on Link with no source evidence at all.
    $unknown = Write-Cards @(
        @{tool='create_successor';arguments=@{at=$cards[0];kind='side';title='谈判地位决定事后分配';body="依赖 [[$anchor]] 的结论：事后分配由地位决定。"}}
    )
    # No I7: a Card filed in that Literature tree rests on it by design.
    $inBook = Write-Cards @(
        @{tool='create_successor';arguments=@{at=$anchor;kind='side';title='企业家用控制权换取融资';body="依赖 [[$anchor]] 的设定：控制权随状态转移。`n`n来源：[[$key]] §1"}}
    )
    $owned = @($cards + $unknown + $inBook)
    $rules = 'R20,R21,I7'
    $lines = @(Invoke-ZtCheck -Cards ($owned -join ',') -Rules $rules)
    $r20 = @($lines | Where-Object { $_ -match ' R20 \(advisory\) ' })
    $r21 = @($lines | Where-Object { $_ -match ' R21 \(advisory\) ' })
    $i7 = @($lines | Where-Object { $_ -match ' I7 ' })
    Assert ($lines[-1] -eq 'faults: 1') "advice is printed but not counted (one hard I7 fault): $($lines -join ' | ')"
    Assert ($r20.Count -eq 1 -and $r20[0].StartsWith("$($cards[0]) R20 (advisory) local reference ``式 (12)`` names no content: 式 (12) 有该项")) 'R20 names the label that stands in for its content'
    Assert ($r21.Count -eq 3) "R21 reports the bare deictics only: $($r21 -join ' | ')"
    Assert (@($r21 | Where-Object { $_ -match '该卡给出了结论' }).Count -eq 1 -and @($r21 | Where-Object { $_ -match '该卡说明了均衡' }).Count -eq 1 -and @($r21 | Where-Object { $_ -match '该卡给出了结果' }).Count -eq 1) 'R21 reports the title and each unnamed sentence, never the sentence that names a Link'
    Assert (@($r21 | Where-Object { $_ -match '有该项' }).Count -eq 0) 'R21 leaves code, quotations, and inline code alone'
    $tick = [char]96
    $ownLine = $cards[3] + ' I7 ' + $tick + 'rests-on' + $tick + " [[$anchor]] enters its own source " + $tick + $key + $tick
    Assert ($i7.Count -eq 2 -and @($i7 | Where-Object { $_ -eq $ownLine }).Count -eq 1) "I7 reports a rests-on Link into the Card own source tree on the audited evidence: $($i7 -join ' | ')"
    Assert (@($i7 | Where-Object { $_ -match 'source evidence unknown' }).Count -eq 1 -and @($i7 | Where-Object { $_ -eq "$($unknown[0]) I7 (advisory) source evidence unknown, so [[$anchor]] may be an own-source premise" }).Count -eq 1) 'missing source evidence is reported as unknown, never as a pass'
    $notOwn = @($cards[4], $cards[5], $inBook[0])
    Assert (@($i7 | Where-Object { $notOwn -contains ($_ -split ' ')[0] }).Count -eq 0) 'an illustrative Link, another book, and a Card in that Literature tree raise no own-source fault'
    # The same rules through the scoped audit: advice apart from blocking style faults.
    # docs/literature-only-digest-design.md: the audit reviews the Cards of the book's Literature tree
    # only (a Topic-tree Card belongs to no book), so the advice Cards for it are filed there.
    $null = Write-Cards @(
        @{tool='create_successor';arguments=@{at=$anchor;kind='side';title='剩余控制权决定契约空白处的用法';body="式 (12) 有该项。`n`n该卡给出了结论。`n`n来源：[[$key]] §1"}}
    )
    $out = Join-Path $scratch 'audit'
    $auditLines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Key2026 -Out $out -SkipModels 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "audit failed: $auditLines" }
    $report = Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    $book = $report.books[0]
    $faults = [IO.File]::ReadAllText((Join-Path $out 'audit-faults.txt'))
    Assert ($book.style.advisory -ge 2 -and $book.style.advice_by_rule.R21 -ge 1 -and $book.style.advice_by_rule.R20 -ge 1) 'the audit counts the standalone advice apart by rule'
    Assert ($book.style.faults -eq 0) "the standalone advice is no blocking style fault: $($book.style | ConvertTo-Json -Compress)"
    # A Card in its own book's Literature tree rests on it by design (no I7), and the Topic-tree
    # Cards with own-source premises belong to no book, so the book audit has no I7 line.
    Assert (-not $book.layer.by_rule.I7 -and $faults -notmatch ' I7 ') 'the book audit reports no own-source premise for the Cards of its Literature tree'
    # The layer fault count holds only blocking lines, and the advisory count holds every
    # published "(advisory)" line.
    $layerLines = @($faults -split "\r?\n" | Where-Object { $_ -match '^\S+ [RIELF]\d+ ' -and $_ -notmatch '^\S+ R\d+ ' })
    $layerAdvice = @($layerLines | Where-Object { $_ -match ' \(advisory\) ' })
    Assert ($book.layer.advisory -eq $layerAdvice.Count -and $book.layer.faults -eq ($layerLines.Count - $layerAdvice.Count)) "the layer report separates advice from blocking faults: $($book.layer | ConvertTo-Json -Compress)"
    $md = Get-Content -LiteralPath (Join-Path $out 'audit.md') -Raw -Encoding utf8
    Assert ($md -match 'Standalone advice \(R20, R21\): \d+ by rule:.*R2[01]') 'audit.md names the standalone advice apart from the blocking language faults'
    # Read-only: the archive and every Card are exactly as they were.
    $status = @(zt status)
    Assert (@($status | Where-Object { $_ -match '^state: up$' }).Count -eq 1) 'the audit left the service up'
    zt status --check-links | Out-Null
    Assert ($LASTEXITCODE -eq 0) 'the link index still matches every stored Reverse section'
} finally { zt down | Out-Null }
