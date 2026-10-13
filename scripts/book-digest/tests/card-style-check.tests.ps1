#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\card-style-check.tests.ps1
# Checks card-style-check.ps1 against a fixture with one fault for each rule it checks, and
# text it must leave alone (formulas, Links, tables, quotations, charts).
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$script = Join-Path $PSScriptRoot '..\card-style-check.ps1'
$fixture = Join-Path $PSScriptRoot 'card-style-fixture.json'
$lines = @(pwsh -NoProfile -File $script -Json $fixture -Setting 'K/1|a|a')
$expected = @(
    'K/1|a|a|1 R12 (49 characters > 40) 清算比例：E 付不出 P 时'
    'K/1|a|a|1 R3 E 在时点 1 付不出去；E 能部分付出。'
    'K/1|a|a|1 R4 资产被清算。'
    'K/1|a|a|1 R5 双方进行谈判。'
    'K/1|a|a|1 R7 E 在时点 1 的用来还债的现金的上限是 $P$。'
    'K/1|a|a|1 R6 uses `卖资产` for `清算`: 值得注意的是，C 会卖资产。'
    'K/1|a|a|1 R9 `值得注意的是`: 值得注意的是，C 会卖资产。'
    'K/1|a|a|1 R6 uses `还款额` for `可承诺的还款`: 可承诺的还款等于 $B$，还款额不会更高。'
    'K/1|a|a|1 R2 (50 characters > 40) 这一句很长很长'
    'K/1|a|a|2 R2 (34 characters > 30) 代入 $P=B$ 以后'
    'K/1|a|a|3 R3 The creditor cannot verify the return; it cannot enforce a payment from it either.'
    'K/1|a|a|4 R13 (13 sentences > 12) split the part'
    'K/1|a|a|5 R14 (7 steps > 6) split the derivation into a chain of Derivation parts'
    'faults: 13'
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
# The setting's own tables, the quotation, chart, and formulas, and a shared part's words under
# another model's term table raise nothing.
foreach ($line in $lines) {
    if ($line -match '引文|图里|清算价值；|^Other/') { Write-Output "checked text it must skip: $line"; $failed++ }
}
# R13 counts prose characters too, against -MaxChars; a Derivation part's steps count under R14 only.
$small = Join-Path ([IO.Path]::GetTempPath()) "card-style-size-$PID.json"
@(
    [pscustomobject]@{ address = 'S/1|a|a'; facet = '设定'; title = '设定'; body = 'E 有一个项目，项目在时点 2 产生收益。C 出资，换取时点 1 的还款。E 可以挪用一部分收益。' }
    [pscustomobject]@{ address = 'S/1|a|a|1'; facet = '推导'; title = '推导'; body = "1. E 在时点 1 还款以后，手里剩下项目的收益。`n2. E 在时点 2 挪用一部分收益，留下其余部分。`n3. 两边加上初始财富，得到参与约束。" }
) | ConvertTo-Json | Set-Content -LiteralPath $small -Encoding utf8
$size = @(pwsh -NoProfile -File $script -Json $small -MaxChars 30 -MaxSteps 2)
# A concept model keeps its few parts whole: no size faults.
$concept = @(pwsh -NoProfile -File $script -Json $small -MaxChars 30 -MaxSteps 2 -Level concept)
Remove-Item -LiteralPath $small
if ($concept[-1] -ne 'faults: 0') { Write-Output "a concept model must raise no size fault:"; $concept | ForEach-Object { Write-Output "  $_" }; $failed++ }
$expectedSize = @(
    'S/1|a|a R13 ('
    'S/1|a|a|1 R14 (3 steps > 2)'
    'faults: 2'
)
if ($size.Count -ne $expectedSize.Count) { Write-Output "size check: expected $($expectedSize.Count) lines, got:"; $size | ForEach-Object { Write-Output "  $_" }; $failed++ }
else {
    for ($i = 0; $i -lt $size.Count; $i++) {
        if (-not $size[$i].StartsWith($expectedSize[$i])) { Write-Output "size line $i`n  expected: $($expectedSize[$i])`n  got:      $($size[$i])"; $failed++ }
    }
    if ($size[0] -notmatch 'characters > 30') { Write-Output "size check: R13 must count characters: $($size[0])"; $failed++ }
}
if ($failed) { Write-Output "FAILED: $failed"; exit 1 }
Write-Output 'card-style-check: all checks passed'
