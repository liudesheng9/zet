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
    'faults: 11'
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
# The setting's own tables and the quotation, chart, and formulas raise nothing.
foreach ($line in $lines) {
    if ($line -match '引文|图里|清算价值；') { Write-Output "checked text it must skip: $line"; $failed++ }
}
if ($failed) { Write-Output "FAILED: $failed"; exit 1 }
Write-Output 'card-style-check: all checks passed'
