#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\card-layer.tests.ps1
# Checks the Card layer rules of zt-check (I, E, and L rules) against a fixture: a
# good Idea Card (its Link after 引出 is no L1 fault), a short one with every Idea fault, a long one, a bare Event Card, a good one, a
# second report of the same event, an anchor, and a Model part that names no category (it gets the
# R rules only).
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$fixture = Join-Path $PSScriptRoot 'card-layer-fixture.json'
$lines = @(Invoke-ZtCheck -Json $fixture -MaxIdeaChars 250)
$expected = @(
    '40/1|a I1 (advisory) the title states no claim: 所有权'
    '40/1|a I2 (9 characters < 40) merge candidate'
    '40/1|a I3 no source line'
    '40/1|a I4 no kinded Link'
    '40/1|a I5 no other Card has an authored Link here'
    '40/1|a L1 (1 Links read as refers) [[41/2]]'
    '40/1|a L2 (1 characters < 8) a Link with no reason: 见 [[41/2]]。'
    '40/1|b I2 (283 characters > 250) split candidate'
    '40/1|b L2 (7 characters < 8) a Link with no reason: 依赖 [[40/1]]：同一个机制。'
    '40/1|b L2 (7 characters < 8) a Link with no reason: 对照 [[41/1]]：另一个机制。'
    '40/1|b L3 (advisory) [[41/1]] is linked in 2 paragraphs'
    'Key/3|a E1 no date in the title'
    'Key/3|a E2 missing labels: 时间、地点、参与者、经过'
    'Key/3|a E4 no Idea Card links it with a kind word'
    '40/1|b I6 (advisory) a title very similar to [[40/1]]'
    'Key/3|c E5 (advisory) the same event as [[Key/3|b]]?'
    'faults: 16'
)
$failed = 0
if ($lines.Count -ne $expected.Count) {
    Write-Output "expected $($expected.Count) lines, got $($lines.Count):"
    $lines | ForEach-Object { Write-Output "  $_" }
    $failed++
}
for ($i = 0; $i -lt [Math]::Min($lines.Count, $expected.Count); $i++) {
    if (-not $lines[$i].StartsWith($expected[$i])) {
        Write-Output "line $i`n  expected: $($expected[$i])`n  got:      $($lines[$i])"
        $failed++
    }
}
# The good Cards, the anchor, and the Model part raise nothing; the source line is no Link fault.
foreach ($line in $lines) {
    if ($line -match '^(40/1|Key/3|Key/3\|b|Key/9\|a) ') { Write-Output "a good Card raised: $line"; $failed++ }
    if ($line -match '来源') { Write-Output "a source line raised: $line"; $failed++ }
}
# Without a category, the same Cards get the R rules only (the model compiler's view).
$plain = Join-Path ([IO.Path]::GetTempPath()) "card-layer-plain-$PID.json"
Get-Content -LiteralPath $fixture -Raw -Encoding utf8 | ConvertFrom-Json |
    Select-Object address, title, body, facet, line | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $plain -Encoding utf8
$r = @(Invoke-ZtCheck -Json $plain)
Remove-Item -LiteralPath $plain
if (@($r | Where-Object { $_ -match ' [IEL]\d ' }).Count) { Write-Output 'entries without a category raised layer rules:'; $r | ForEach-Object { Write-Output "  $_" }; $failed++ }
# A formula is a reason: "代入 [[…]] 的定义得 $…$" raises no L2.
$formula = Join-Path ([IO.Path]::GetTempPath()) "card-layer-formula-$PID.json"
@([pscustomobject]@{ address = '43/1'; category = 'idea'; title = '边际技术替代率等于两种投入之比'; parent = '43/0'; links = @('41/2', 'Key'); kinds = @('41/2')
        body = "代入 [[41/2]] 的定义得 `$MRTS=\alpha z_2/(\beta z_1)`$。`n`n来源：[[Key]] §3" }) |
    ConvertTo-Json -Depth 4 -AsArray | Set-Content -LiteralPath $formula -Encoding utf8
$fr = @(Invoke-ZtCheck -Json $formula)
Remove-Item -LiteralPath $formula
if (@($fr | Where-Object { $_ -match ' L2 ' }).Count) { Write-Output 'a Link whose sentence holds a formula was called reasonless:'; $fr | ForEach-Object { Write-Output "  $_" }; $failed++ }
# A `|` belongs to the address: [[41/1|a]] and [[41/1|b]] are two targets, so no L3.
$pipe = Join-Path ([IO.Path]::GetTempPath()) "card-layer-pipe-$PID.json"
@([pscustomobject]@{ address = '42/1'; category = 'idea'; title = '控制权随状态转移给债权人'; parent = '42/0'; links = @('41/1|a', '41/1|b', 'Key'); kinds = @('41/1|a', '41/1|b')
        body = "违约时债权人取得控制权。对照 [[41/1|a]]：那里控制权留在企业家手里。`n`n对照 [[41/1|b]]：那里控制权按投票分配。`n`n来源：[[Key]] §2" }) |
    ConvertTo-Json -Depth 4 -AsArray | Set-Content -LiteralPath $pipe -Encoding utf8
$p = @(Invoke-ZtCheck -Json $pipe)
Remove-Item -LiteralPath $pipe
if (@($p | Where-Object { $_ -match ' L3 ' }).Count) { Write-Output 'two addresses that differ after | were read as one target:'; $p | ForEach-Object { Write-Output "  $_" }; $failed++ }
if ($failed) { Write-Output "FAILED: $failed"; exit 1 }
Write-Output 'card-layer: all checks passed'
