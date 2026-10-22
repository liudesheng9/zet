#Requires -Version 7
# Run: pwsh -NoProfile -File scripts\book-digest\tests\book-terms.tests.ps1 [-Zt <zt.exe>]
# Drives book-terms.ps1 (plan, apply) on a scratch archive and a scratch staging folder, standing in
# for DeepSeek's terms.json, then checks that digest-audit.ps1 finds the term table and applies
# rule R6 to an Idea Card of the book that uses a word the table rejects.
param([string]$Zt = '')
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$scratch = Join-Path ([IO.Path]::GetTempPath()) "book-terms-test-$PID"
$archive = Join-Path $scratch 'archive'
$staging = Join-Path $scratch 'staging'
New-Item -ItemType Directory -Force $archive, (Join-Path $staging 'sections') | Out-Null
if ($Zt) {
    $bin = Join-Path $scratch 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $Zt -Destination (Join-Path $bin 'zt.exe')
    $env:PATH = "$bin;$env:PATH"
}
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
$terms = Join-Path $PSScriptRoot '..\book-terms.ps1' | Resolve-Path | ForEach-Object Path
$audit = Join-Path $PSScriptRoot '..\digest-audit.ps1' | Resolve-Path | ForEach-Object Path
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
$failed = 0
function Assert($condition, $message) { if (-not $condition) { Write-Output "FAIL: $message"; $script:failed++ } else { Write-Output "ok: $message" } }
function Last($lines) { [string]@($lines)[-1] | ConvertFrom-Json -AsHashtable }
function Write-Calls($calls) {
    $out = @(($calls | ConvertTo-Json -Depth 6 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "zt: $out" }
    , @($out | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
$key = 'Terms2024'
try {
    zt config set archive_root $archive | Out-Null
    zt up | Out-Null
    $null = Write-Calls @(
        @{ tool = 'create_literature'; arguments = @{ workspace = 'agent'; bibtex = "@book{$key, title={Terms}, author={Hart, Oliver}, year={2024}}"; body = '全书索引。' } },
        @{ tool = 'create_successor'; arguments = @{ at = $key; kind = 'direct'; title = '第 1 组：所有权决定剩余控制权'; body = '§1 - 所有权' } }
    )
    # docs/literature-only-digest-design.md: the book's Idea Card is in its Literature tree (a Topic Card belongs to no book).
    $idea = (Write-Calls @(@{ tool = 'create_successor'; arguments = @{ at = "$key/1"; kind = 'side'; title = '所有者在契约空白处决定资产用法'
                    body = "所有者掌握残余控制权。依赖 [[$key/1]]：契约不完全。`n`n来源：[[$key]] §1" } }))[0]
    [IO.File]::WriteAllText((Join-Path $staging 'sections\s01.md'), "# 第一章`n`n我们把契约没有写明时决定资产用法的权利称为**剩余控制权**。`n`n**敲竹杠**是专用投资之后的机会主义。`n", $utf8)
    [IO.File]::WriteAllText((Join-Path $staging 'manifest.json'), (@{ citation_key = $key; book_title = '术语测试'; groups = @(@{ n = 1; chapter = '第一章'; headings = @('所有权') }) } | ConvertTo-Json -Depth 5), $utf8)

    $plan = Last @(pwsh -NoProfile -File $terms -Step plan -Staging $staging 2>&1)
    Assert ($plan.bold -eq 2 -and $plan.definitions -eq 1 -and @($plan.jobs).Count -eq 1) "plan: two bold terms, one definition, one job ($($plan.bold), $($plan.definitions))"
    $job = Get-Content (Join-Path $staging 'terms\job.md') -Raw -Encoding utf8
    Assert ($job -match '剩余控制权（`sections/s01.md:3`）' -and $job -match '称为') 'plan: the job lists the terms with their lines'
    Assert ((Get-Content (Join-Path $staging 'terms\dsh-terms-instructions.zh.md') -Raw -Encoding utf8) -match "\[\[$key\]\]") 'plan: the instructions name the book'

    # DeepSeek stand-in: two good terms, a duplicate, an empty term, and an avoid word inside its term.
    @{ terms = @(
            @{ term = '剩余控制权'; avoid = @('残余控制权', '剩余控制权', '控制'); meaning = '契约没有写明时决定资产用法的权利'; section = '§1' },
            @{ term = '敲竹杠'; avoid = @('要挟'); meaning = '专用投资之后的机会主义'; section = '§1' },
            @{ term = '剩余控制权'; avoid = @('别的'); meaning = '重复'; section = '§1' },
            @{ term = ''; avoid = @('空'); meaning = '空'; section = '' }
        ) } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $staging 'terms\out\terms.json') -Encoding utf8
    $applied = Last @(pwsh -NoProfile -File $terms -Step apply -Staging $staging -Apply 2>&1)
    Assert ($applied.ok -and $applied.terms -eq 2) "apply: two valid terms ($($applied.terms))"
    $card = @(Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = $applied.card } }))[0]
    Assert ($card.title -match '^术语表' -and $card.parent.address -eq "$key/1") "apply: the 术语表 Card is a Side successor of $key/1 ($($applied.card))"
    Assert ($card.body -match '\| 剩余控制权 \| 残余控制权 \|' -and $card.body -notmatch '剩余控制权、' -and $card.body -notmatch '\| 控制 ') 'apply: the table drops the term itself and a word inside it'
    $again = Last @(pwsh -NoProfile -File $terms -Step apply -Staging $staging -Apply 2>&1)
    Assert ($again.card -eq $applied.card) 'apply: a second run rewrites the same Card'

    $out = Join-Path $scratch 'audit'
    $null = @(pwsh -NoProfile -File $audit -CitationKey $key -Out $out 2>&1)
    $report = Get-Content (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $book = @($report.books)[0]
    Assert ($book.layer.term_table -eq $applied.card) 'audit: finds the term table'
    Assert ($book.cards_by_category.terms -eq 1) 'audit: the term table is its own category'
    $faults = @(Get-Content (Join-Path $out 'audit-faults.txt') -Encoding utf8)
    Assert (@($faults | Where-Object { $_ -match "^$([regex]::Escape($idea)) R6 uses ``残余控制权`` for ``剩余控制权``" }).Count -eq 1) "audit: R6 flags the rejected word in the Idea Card ($($faults -join ' | '))"
    Assert (-not @($faults | Where-Object { $_ -match "^$([regex]::Escape($applied.card)) " }).Count) 'audit: the term table itself raises nothing'
} finally {
    zt down 2>&1 | Out-Null
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output "failed: $failed"
if ($failed) { exit 1 }
