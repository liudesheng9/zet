#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\zt-calls.tests.ps1 [-Script <list-models.ps1 to test>]
# Regression test: a book-digest script must read the zt helper's UTF-8 output as UTF-8 even when
# its parent's console code page is GBK (936). It once decoded the output as GBK, and the Chinese
# excerpt below swallowed a quote and broke the JSON of list_trees. Uses a scratch archive: it sets
# ZT_CONFIG_DIR, starts a service there, and stops it at the end. The live archive is not touched.
param([string]$Script = (Join-Path $PSScriptRoot '..\list-models.ps1'))
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "zt-calls-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
New-Item -ItemType Directory -Force $archive, $staging | Out-Null
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$helper = Join-Path $PSScriptRoot '..\..\..\skills\zt\scripts\zt-mcp.ps1' | Resolve-Path | ForEach-Object Path
$failed = 0
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $excerpt = '本主题追问：上市公司的债务与股权比例由什么决定，债务又靠什么机制约束管理层。代理视角给出的答案是一个权衡：债务强制现金流出、遏制帝国扩张，却在坏状态下带来无效清算与债务悬置，最优资本结构就落在两者之间；它还解释了税收与信息理论解释不了的债务之“硬”。'
    $calls = @(
        @{ tool = 'create_topic'; arguments = @{ title = '资本结构'; body = $excerpt } },
        @{ tool = 'create_literature'; arguments = @{ bibtex = '@book{Test2020, title={Firms}}'; body = $excerpt } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020'; title = '第一章' } },
        @{ tool = 'create_successor'; arguments = @{ at = 'Test2020/1'; kind = 'side'; type = 'model'; title = '债务约束管理层：“硬”债务遏制帝国扩张'; body = $excerpt } }
    )
    $out = ($calls | ConvertTo-Json -Depth 5 -AsArray) | pwsh -NoProfile -File $helper -
    if ($LASTEXITCODE -ne 0) { throw "setup failed: $out" }

    # The parent reads the script's output in the console's code page, as a workflow agent does.
    [Console]::OutputEncoding = [Text.Encoding]::GetEncoding(936)
    $lines = @(pwsh -NoProfile -File $Script -Staging $staging -All -CitationKey Test2020 2>&1)
    [Console]::OutputEncoding = $utf8
    $last = [string]$lines[-1]
    try {
        $models = @(($last | ConvertFrom-Json).models)
        if ($models.Count -ne 1 -or $models[0].address -ne 'Test2020/1|a') {
            Write-Output "expected one model Test2020/1|a, got: $last"; $failed++
        }
    } catch {
        Write-Output "the script's last line is not JSON:"; $lines | Select-Object -Last 6 | ForEach-Object { Write-Output "  $_" }; $failed++
    }
} finally {
    [Console]::OutputEncoding = $utf8
    zt down 2>$null | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { Write-Output "FAILED: $failed"; exit 1 }
Write-Output 'zt-calls: list-models reads UTF-8 under a GBK console'
