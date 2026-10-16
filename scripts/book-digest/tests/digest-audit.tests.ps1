#Requires -Version 7.5
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$scratch = Join-Path ([IO.Path]::GetTempPath()) "bridge-audit-test-$PID"
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
    $made = Write-Cards @(
        @{tool='create_literature';arguments=@{bibtex='@book{Audit2026,title={Audit},year={2026}}'}},
        @{tool='create_successor';arguments=@{at='Audit2026';title='第一组';body='§1 企业'}},
        @{tool='create_successor';arguments=@{at='Audit2026/1';kind='side';type='finding';title='企业因为融资约束减少投资';body="测量：企业投资。`n数据：企业调查。`n方法：样本比较。`n结果：受约束的企业投资较少。`n局限：样本较小。`n来源：[[Audit2026]] §1"}}
    )
    $out = Join-Path $scratch 'audit'
    $lines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Audit2026 -Out $out -SkipModels 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "audit failed: $lines" }
    $report = Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($report.books[0].cards_by_category.finding -eq 1 -and $report.books[0].cards -eq 3) 'the public audit reports a Finding Card separately from roots and anchors'
    $more = Write-Cards @(
        @{tool='create_literature';arguments=@{bibtex='@book{Earlier2020,title={Earlier},year={2020}}'}},
        @{tool='create_successor';arguments=@{at='Earlier2020';title='第一组';body='§1 企业'}},
        @{tool='create_successor';arguments=@{at='Earlier2020/1';kind='side';title='融资约束影响投资';body="投资取决于资金。`n`n对应 [[$($made[2])]]：两张卡都讨论融资约束降低投资。"}},
        @{tool='edit_card';arguments=@{at=$made[2];body="融资约束影响投资。`n`n对应 [[@@2]]：两处都由资金限制决定投资规模。`n`n再次对应 [[@@2]]：同一方向的重复链接只算一对。"}}
    )
    $lines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Audit2026 -Out $out -SkipModels 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "audit failed: $lines" }
    $report = Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    $bridges=$report.books[0].bridges
    Assert ($bridges.complete -and -not $bridges.island -and $bridges.out.books.Earlier2020 -eq 1 -and $bridges.in.books.Earlier2020 -eq 1) 'a filtered audit counts distinct actual semantic pairs in both directions'
    $extra=Write-Cards @(
        @{tool='create_successor';arguments=@{at='Audit2026/1';kind='side';type='model';title='控制权模型';body='控制权决定投资。'}},
        @{tool='create_successor';arguments=@{at='@@0';kind='side';type='model';facet='设定';title='模型设定';body="对应 [[$($more[2])]]：两处都由融资约束改变投资规模。"}},
        @{tool='create_topic';arguments=@{title='审计枢纽';body='枢纽入口'}},
        @{tool='create_successor';arguments=@{at='@@2';type='hub';level='topic';title='融资约束枢纽';body="## 关系`n- 对应 [[$($made[2])]]：这里的索引指向融资约束。"}},
        @{tool='create_successor';arguments=@{at='Audit2026/1';kind='side';title='符号表';body="对应 [[$($more[2])]]：这是符号表的导航说明。"}},
        @{tool='create_successor';arguments=@{at='Audit2026/1';kind='side';title='一个普通引用';body="文献另见 [[$($more[2])]]。"}}
    )
    $card=(Invoke-Zt @(@{tool='get_card';arguments=@{at=$made[2]}}))[0]
    Write-Cards @(@{tool='edit_card';arguments=@{at=$made[2];body=($card.body+"`n`n对应 [[Earlier2020]]：来源引用不构成桥接。`n对应 [[Earlier2020/1]]：锚点导航不构成桥接。`n对应 [[$($extra[3])]]：枢纽导航不构成桥接。")}})|Out-Null
    $lines=@(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Audit2026 -Out $out -SkipModels 2>&1)
    if($LASTEXITCODE){throw "audit failed: $lines"}
    $report=Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw -Encoding utf8|ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($report.books[0].bridges.out.books.Earlier2020 -eq 2 -and $report.books[0].bridges.in.books.Earlier2020 -eq 1) 'actual Model-part semantics count while roots, anchors, hubs, notation and plain references do not'
} finally { zt down | Out-Null }
