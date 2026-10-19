#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\workspace-titles.tests.ps1
# #93 S03: I6 compares a checked Idea title with every title of its Workspace (not only its
# own tree), reports an unordered pair once, and never turns a context Card into a repair
# target. A real scratch archive and the public `zt-check --cards` contract; no provider runs.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "workspace-titles-test-$PID"
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, [string]$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper -)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $lines" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    # Two Topic trees in the agent Workspace, each with one Idea Card; the second title is the
    # first with one extra 的, so the existing Dice function puts them above 0.8.
    $first = Write-Cards @(
        @{tool='create_topic';arguments=@{title='剩余控制权';body='谁在契约空白处做决定？'}},
        @{tool='create_successor';arguments=@{at='@@0';title='剩余控制权决定契约空白处';body='所有者拒绝合作的能力决定契约没有写明的部分。'}}
    )
    $second = Write-Cards @(
        @{tool='create_topic';arguments=@{title='不完全契约';body='契约为什么总留下空白？'}},
        @{tool='create_successor';arguments=@{at='@@0';title='剩余控制权决定契约的空白处';body='同一机制换了措辞，写在另一条主题树上。'}}
    )
    $one, $two = $first[1], $second[1]
    # Both Cards checked: one unordered pair, reported once.
    $both = @(Invoke-ZtCheck -Cards "$one,$two" -Rules 'I6')
    $pair = @($both | Where-Object { $_ -match '^' + [regex]::Escape($two) + ' I6 \(advisory\) a title very similar to \[\[' + [regex]::Escape($one) + '\]\]$' })
    Assert ($pair.Count -eq 1 -and @($both | Where-Object { $_ -match ' I6 ' }).Count -eq 1) "precheck: two checked similar titles give exactly one pair line ($($both -join ' | '))"
    # Only the first Card checked: the second is Workspace context, not just its own tree.
    $onlyOne = @(Invoke-ZtCheck -Cards $one -Rules 'I6')
    Assert (@($onlyOne | Where-Object { $_ -eq "$one I6 (advisory) a title very similar to [[$two]]" }).Count -eq 1) "the comparison reaches another tree of the same Workspace ($($onlyOne -join ' | '))"
    # Only the second Card checked: the first is context, and no line names the un-checked Card.
    $onlyTwo = @(Invoke-ZtCheck -Cards $two -Rules 'I6')
    Assert (@($onlyTwo | Where-Object { $_ -eq "$two I6 (advisory) a title very similar to [[$one]]" }).Count -eq 1) 'the other direction of the same pair is reported on the checked Card'
    Assert (@($onlyTwo | Where-Object { $_ -match ('^' + [regex]::Escape($one) + ' ') }).Count -eq 0) 'a context Card is never the subject of a fault line'
    # A dissimilar title in the same Workspace raises nothing.
    $other = Write-Cards @(@{tool='create_topic';arguments=@{title='资产专用性';body='专用性投资怎样影响合作？'}})
    $unrelated = Write-Cards @(@{tool='create_successor';arguments=@{at=$other[0];title='专用性投资提高合作的剩余';body='投资转用以后价值下降，双方因此愿意合作。'}})
    $none = @(Invoke-ZtCheck -Cards $unrelated[0] -Rules 'I6')
    Assert (@($none | Where-Object { $_ -match ' I6 ' }).Count -eq 0) "an unrelated title raises no I6 ($($none -join ' | '))"
    # Read-only: the checker changed no Card and left one revision.
    $revision = @(zt status | Where-Object { $_ -match '^revision: ' })
    Assert ($revision.Count -eq 1) 'the checker leaves the archive service record intact'
    zt status --check-links | Out-Null
    Assert ($LASTEXITCODE -eq 0) 'the link index still matches every stored Reverse section'
} finally { zt down | Out-Null }
