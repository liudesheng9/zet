#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\coverage-audit.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# L5 (#188): the audit lists a prepared run's uncovered Literature-note units and its C3 numbers as
# findings, and neither fails the audit. One scratch archive, one prepared staging folder whose
# source log covers only the first paragraph, and the number on the note is not on that line.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "coverage-audit-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, [string]$message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $($lines -join ' | ')" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Run-Audit([string]$out, [string]$key) {
    $arguments = @('-NoProfile', '-File', (Join-Path $PSScriptRoot '..\digest-audit.ps1'), '-Out', $out)
    if ($key) { $arguments += @('-CitationKey', $key) }
    $lines = @(& pwsh @arguments 2>&1)
    if ($LASTEXITCODE) { throw "audit failed: $($lines -join ' | ')" }
    return Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $made = Write-Cards @(
        @{tool='create_literature';arguments=@{bibtex='@book{CovBook,title={Coverage Book},year={2026}}'}},
        @{tool='create_successor';arguments=@{at='CovBook';title='第一章';body='第一章的正文，长度超过二十个字，是一个段落单元。'}},
        @{tool='create_successor';arguments=@{at='@@1';kind='side';title='第一段的结论';body="作者给出 63 倍的结果，这个数字不在所引用的原文行里。`n`n来源：[[CovBook]] §1"}}
    )
    $note = $made[2]

    # The prepared staging folder: two paragraph units, a span that covers only the first.
    $staging = Join-Path $scratch 'staging'
    New-Item -ItemType Directory -Force (Join-Path $staging 'sections') | Out-Null
    @{
        citation_key = 'CovBook'
        groups = @(@{ n = 1; id = 'g01'; anchor = 'CovBook/1'; sections = @('s01.md') })
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $staging 'manifest.json') -Encoding utf8
    @(
        '第一段的正文，长度超过二十个字，这里没有数字。',
        '',
        '第二段的正文，长度超过二十个字，也没有数字，这一段没有卡片覆盖。',
        ''
    ) -join "`n" | Set-Content -LiteralPath (Join-Path $staging 'sections\s01.md') -Encoding utf8
    "$note`tsections/s01.md:1" | Set-Content -LiteralPath (Join-Path $staging 'src-g01.txt') -Encoding utf8

    $report = Run-Audit $staging 'CovBook'
    Assert ($report.complete -eq $true) 'coverage audit: the read-only audit still completes with an uncovered unit and a C3 number'
    $coverage = $report.coverage
    Assert ($coverage -and $coverage.status -ceq 'complete' -and $coverage.book -ceq 'CovBook') "coverage audit: the prepared run's coverage is reported ($($coverage.status))"
    Assert ($coverage.units -eq 2 -and $coverage.covered -eq 1 -and $coverage.uncovered -eq 1 -and $coverage.pct -eq 50) "coverage audit: the counts come from the paragraph units and the recorded span ($($coverage.covered)/$($coverage.units), $($coverage.pct)%)"
    Assert (@($coverage.uncovered_units).Count -eq 1 -and @($coverage.uncovered_units)[0].section -ceq 's01.md' -and @($coverage.uncovered_units)[0].line -ceq '3' -and @($coverage.uncovered_units)[0].text -match '第二段') 'coverage audit: the uncovered unit is listed with its section, lines and text'
    $numbers = @($coverage.c3 | Where-Object { $_.address -ceq $note } | ForEach-Object { $_.number })
    Assert ($numbers -contains '63' -and $numbers -contains '1') "coverage audit: the numbers on the note are listed as findings ($($numbers -join ', '))"
    Assert ($report.books[0].coverage -and $report.books[0].coverage.units -eq 2) 'coverage audit: the finding belongs to the book the staging folder names'
    $md = Get-Content -LiteralPath (Join-Path $staging 'audit.md') -Raw -Encoding utf8
    Assert ($md -match '\*\*Coverage\*\* \(Literature notes\): 2 units: 1 covered' -and $md -match 'Uncovered units:.*s01\.md:3' -and $md -match 'C3 numbers:.*63') 'coverage audit: audit.md lists the uncovered unit and the C3 number'

    # A folder that is no prepared run has no coverage finding, never a zero that stands in for it.
    $plain = Join-Path $scratch 'plain'
    $plainReport = Run-Audit $plain 'CovBook'
    Assert ($null -eq $plainReport.coverage) 'coverage audit: a folder that is no prepared run carries no coverage finding'
    Write-Output 'coverage audit: ok'
} finally {
    $env:ZT_DIGEST = ''
    if (Get-Command zt -ErrorAction SilentlyContinue) { zt down 2>&1 | Out-Null }
}
