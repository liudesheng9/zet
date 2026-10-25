#Requires -Version 7.5
# Run: pwsh -NoProfile -File scripts\book-digest\tests\term-audit.tests.ps1 [-Zt <zt.exe>] [-Digest <zt-digest.exe>]
# S26 (#169, R0441/R0442): the audit's `terms` section at its real command boundary: one scratch
# archive with a Term list, a concept head, a math head with its continuation part, a Term use, an
# avoid word still written plainly, and Cards with no Term use. It checks the coverage, the
# math/concept counts, the T faults, the uses by Card type, the no-use observations, the ten most
# used terms, the mapping fingerprints the finalization reads, the run generation, the check
# contract, and that a Term debt degrades the terms status without touching the audit's completion.
param([string]$Zt = '', [string]$Digest = $env:ZT_DIGEST)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) "term-audit-test-$PID"))
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
if (-not $scratch.StartsWith($tempRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'scratch escaped the temp directory' }
New-Item -ItemType Directory -Force $scratch | Out-Null
if ($Zt) { $env:PATH = "$(Split-Path -Parent $Zt);$env:PATH" }
if (-not $Digest) { $Digest = (Get-Command zt-digest -CommandType Application | Select-Object -First 1).Source }
$env:ZT_DIGEST = $Digest
$env:ZT_CONFIG_DIR = Join-Path $scratch 'config'
$env:ZT_GUI_PORT = '0'
. (Join-Path $PSScriptRoot '..\zt-calls.ps1')
function Assert($condition, $message) { if (-not $condition) { throw $message }; Write-Output "ok: $message" }
function Write-Cards($calls) {
    $lines = @(($calls | ConvertTo-Json -Depth 20 -AsArray) | pwsh -NoProfile -File $script:ZtHelper - -Open)
    if ($LASTEXITCODE -ne 0) { throw "fixture create failed: $($lines -join ' | ')" }
    , @($lines | Where-Object { $_ -match '^\d+ ok ' } | ForEach-Object { ($_ -split ' ok ', 2)[1].Trim() })
}
function Run-Audit($out) {
    $lines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey TermBook -Out $out -SkipModels 2>&1)
    if ($LASTEXITCODE) { throw "audit failed: $($lines -join ' | ')" }
    return Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
}
try {
    zt config set archive_root (Join-Path $scratch 'archive') | Out-Null
    zt up | Out-Null
    $made = Write-Cards @(
        @{tool='create_literature';arguments=@{bibtex='@book{TermBook,title={Term Book},year={2026}}'}},
        @{tool='create_successor';arguments=@{at='TermBook';title='第一章';body='第一章讲均衡与价格。'}},
        @{tool='create_successor';arguments=@{at='TermBook/1';kind='side';type='term';title='术语表：Term Book'
            body="本卡是《Term Book》的术语表。`n`n术语表`n`n| 术语 | 不用 | 意思 | 首次定义 |`n|---|---|---|---|`n| 均衡 | 平衡 | 供需相等 | §1.1 |`n| 弹性 | — | 价格反应 | §1.2 |`n`n来源：[[TermBook]]"}},
        @{tool='create_successor';arguments=@{at='@@2';kind='side';type='term';term='均衡';title='均衡'
            body="术语：均衡`n类别：概念`n首次定义：§1.1`n`n## 释义`n`n双方都不改变策略。`n`n来源：[[TermBook]] §1.1"}},
        @{tool='create_successor';arguments=@{at='@@2';kind='side';type='term';term='弹性';title='弹性'
            body="术语：弹性`n类别：数学概念`n首次定义：§1.2`n`n## 释义`n`n一个量对另一个量的反应。`n`n## 数学定义`n`n`$`$e=\\frac{dq}{dp}`$`n`n来源：[[TermBook]] §1.2"}},
        @{tool='create_successor';arguments=@{at='@@3';kind='direct';type='term';term='均衡';title='推导（续 1/1）：均衡'
            body="类别：概念`n续：[[@@3]]`n`n## 推导`n`n1. 由定义直接得到。`n`n来源：[[TermBook]] §1.1"}},
        @{tool='create_successor';arguments=@{at='TermBook/1';kind='side';title='均衡决定价格'
            body="均衡 [[均衡@@@3]] 决定价格，平衡这个词也出现在这里。`n`n来源：[[TermBook]] §1.1"}},
        @{tool='create_successor';arguments=@{at='TermBook/1';kind='side';title='弹性决定反应'
            body="这里没有术语链接，只有价格。`n`n来源：[[TermBook]] §1.2"}}
    )
    $list, $conceptHead, $mathHead, $part, $useCard, $plainCard = $made[2..7]

    # --- the complete Term section ------------------------------------------------------------------
    $out = Join-Path $scratch 'audit'
    $report = Run-Audit $out
    $terms = $report.books[0].terms
    Assert ($report.complete -and $report.books[0].terms.status -ceq 'complete') "terms: the audit ran and the book's Term quality is complete ($($terms.status))"
    Assert ($terms.list -ceq $list -and $terms.title -match '术语表') "terms: the section names the Term list ($($terms.list))"
    Assert ($terms.rows -eq 2 -and $terms.published -eq 2 -and $terms.missing -eq 0) "terms: every row publishes a head ($($terms.published)/$($terms.rows))"
    Assert ($terms.heads -eq 2 -and $terms.parts -eq 1 -and $terms.unfiled -eq 0) "terms: two heads and one continuation part, none unfiled"
    Assert ($terms.math -eq 1 -and $terms.concept -eq 1) "terms: the math/concept categories come from the authored 类别 lines ($($terms.math) math, $($terms.concept) concept)"
    $detail = @($terms.heads_detail | Where-Object { $_.address -ceq $mathHead })[0]
    Assert ($detail.category -ceq 'math' -and $detail.auto_link -eq $true) 'terms: the math head carries its category and auto-link choice'
    Assert ($terms.uses.total -eq 1 -and $terms.uses.cards -eq 1 -and $terms.uses.by_type.idea -eq 1) "terms: one Term use by an Idea Card ($($terms.uses.by_type.idea))"
    Assert (@($terms.most_used)[0].term -ceq '均衡' -and @($terms.most_used)[0].uses -eq 1) 'terms: the most used terms are ranked by uses'
    Assert (@($terms.no_use.heads).Count -eq 1 -and @($terms.no_use.heads)[0] -ceq $mathHead) "terms: the head with no use is an observation ($(@($terms.no_use.heads) -join ', '))"
    Assert (@($terms.no_use.cards) -contains $plainCard -and @($terms.no_use.cards) -contains $mathHead) 'terms: substantive Cards with no Term use are named'
    Assert ($terms.avoid.count -eq 1 -and @($terms.avoid.list)[0].address -ceq $useCard -and @($terms.avoid.list)[0].word -ceq '平衡' -and @($terms.avoid.list)[0].line -eq 1) "terms: the avoid word left plainly written is named with its Card and line ($($terms.avoid.count))"
    Assert ($terms.faults.total -eq 0) 'terms: a clean Term structure reports no T fault'
    foreach ($fingerprint in @($terms.fingerprints.list, $terms.fingerprints.mapping, $terms.fingerprints.autolink, $terms.fingerprints.vocabulary)) {
        Assert ($fingerprint -match '^[0-9a-f]{64}$' -and $fingerprint.Trim('0').Length -gt 0) 'terms: the mapping fingerprints the finalization reads are published'
    }
    Assert ($terms.generation.available -eq $false -and $terms.generation.reason -match 'no prepared run ledger') 'terms: a folder that is no prepared run says so instead of claiming a generation'
    Assert ($report.workspace.terms.status -ceq 'complete' -and $report.workspace.terms.books_with_list -eq 1 -and $report.workspace.terms.uses -eq 1) 'terms: the Workspace aggregate counts the books with a Term list'
    Assert ($report.gate.rule_version -and $report.gate.stage -ceq 'advisory' -and $report.gate.policy_hash -match '^[0-9a-f]{64}$' -and $report.gate.capabilities.term_links -eq $true) 'gate: the audit carries the running service check contract'
    $cardsArtifact = Get-Content -LiteralPath (Join-Path $out 'audit-cards.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable
    $headRow = @($cardsArtifact | Where-Object { $_.address -ceq $conceptHead })[0]
    Assert ($headRow.type -ceq 'term' -and $headRow.term -ceq '均衡' -and $headRow.term_role -ceq 'card' -and $headRow.term_list -ceq $list -and $headRow.auto_link -eq $true) 'terms: the audited Card state carries the Term facts a finalization round compares'
    $md = Get-Content -LiteralPath (Join-Path $out 'audit.md') -Raw -Encoding utf8
    Assert ($md -match '## Agent Workspace' -and $md -match '\| Terms \| \*\*complete\*\*' -and $md -match 'Most used: 均衡') 'terms: the human report shows the Term coverage and the most used terms'

    # --- a row without a Card degrades the quality, never the audit's completion ---------------------
    $body = "本卡是《Term Book》的术语表。`n`n术语表`n`n| 术语 | 不用 | 意思 | 首次定义 |`n|---|---|---|---|`n| 均衡 | 平衡 | 供需相等 | §1.1 |`n| 弹性 | — | 价格反应 | §1.2 |`n| 边际 | — | 额外一单位 | §2.1 |`n`n来源：[[TermBook]]"
    $null = Write-Cards @(@{tool='edit_card';arguments=@{at=$list;body=$body}})
    $report = Run-Audit $out
    $terms = $report.books[0].terms
    Assert ($report.complete -eq $true -and $terms.status -ceq 'degraded') "terms: a covered row missing its Card degrades the quality while the audit still completes ($($terms.status))"
    Assert ($terms.rows -eq 3 -and $terms.published -eq 2 -and $terms.missing -eq 1) "terms: the missing row is counted, not hidden ($($terms.published)/$($terms.rows))"
    Assert ($terms.faults.by_rule.T8 -ge 1 -and @($terms.faults.list | Where-Object { $_.fault -ceq 'term_row_without_card' }).Count -eq 1) "terms: the T8 coverage fault is reported by rule ($($terms.faults.by_rule.T8))"
    Assert ($terms.reason -match '1/3 rows publish' -or $terms.reason -match 'row') "terms: the degraded status states why ($($terms.reason))"
    Assert ($report.workspace.terms.status -ceq 'degraded') 'terms: the Workspace aggregate takes the worst book status'

    # --- an auto-link opt-out changes the head's choice and the mapping fingerprint -----------------
    $before = (Run-Audit $out).books[0].terms.fingerprints.autolink
    $head = (Invoke-Zt @(@{tool='get_card';arguments=@{at=$conceptHead}}))[0]
    $offline = $head.body.Replace("## 释义", "自动链接：否`n自动链接理由：这个词太一般。`n`n## 释义")
    $null = Write-Cards @(@{tool='edit_card';arguments=@{at=$conceptHead;body=$offline}})
    $report = Run-Audit $out
    $terms = $report.books[0].terms
    $detail = @($terms.heads_detail | Where-Object { $_.address -ceq $conceptHead })[0]
    Assert ($detail.auto_link -eq $false) 'terms: an authored auto-link opt-out is published on the head'
    Assert ($terms.fingerprints.autolink -ne $before) 'terms: the auto-link choice is part of the published mapping fingerprint'

    Write-Output 'all term audit checks passed'
} finally {
    & zt down *> $null
    if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch -ErrorAction SilentlyContinue }
}
