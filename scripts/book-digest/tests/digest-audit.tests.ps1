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
    # Review fixes (2026-10-16): a Finding Card stays in the source and kinded-Link checks, and a book
    # with no substantive Card outside it is not called an island.
    Assert ($report.books[0].no_source.of -eq 1 -and $report.books[0].unkinded.of -eq 1) 'a Finding Card is checked for a source line and a kinded Link'
    Assert ($null -eq $report.books[0].bridges.island -and $report.books[0].bridges.peers -eq 0) 'a book alone in the archive is not reported as an island'
    $md = Get-Content -LiteralPath (Join-Path $out 'audit.md') -Raw -Encoding utf8
    Assert ($md -match '\| Islands \(books with no semantic Bridge in or out\) \| none\. Not known' -and $md -match 'Island: not known \(no Card outside the book\)') 'audit.md names islands and unknown island states'
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
    # A book with peers but no Bridge is an island, and audit.md names it.
    Write-Cards @(@{tool='create_literature';arguments=@{bibtex='@book{Lone2030,title={Lone},year={2030}}'}},@{tool='create_successor';arguments=@{at='Lone2030';title='第一组';body='§1 孤立'}},@{tool='create_successor';arguments=@{at='Lone2030/1';kind='side';title='孤立的观点不链接别的书';body="这一观点只在本书内成立。`n`n来源：[[Lone2030]] §1"}}) | Out-Null
    $lines=@(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Lone2030 -Out $out -SkipModels 2>&1)
    if($LASTEXITCODE){throw "audit failed: $lines"}
    $report=Get-Content -LiteralPath (Join-Path $out 'audit.json') -Raw -Encoding utf8|ConvertFrom-Json -AsHashtable -DateKind String
    $md = Get-Content -LiteralPath (Join-Path $out 'audit.md') -Raw -Encoding utf8
    Assert ($report.books[0].bridges.island -eq $true -and $report.books[0].bridges.peers -gt 0 -and $md -match '\| Islands \(books with no semantic Bridge in or out\) \| `Lone2030`') 'a book with peers and no Bridge is an island that audit.md names'
    # Retain the fda5cf1 report. Adapt only #76's explicit semantic changes:
    # notation is excluded by classification; anchors are not orphan targets.
    $baselineFaults = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'audit-bulk-faults-fixture.txt'))
    $baselineReport = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'audit-bulk-report-fixture.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    $baselineReport.workspace.cards_by_category = [ordered]@{root=4;anchor=3;idea=3;finding=1;hub=1;model=1;notation=1;part=1}
    $baselineReport.books[0].orphans = [ordered]@{count=1;by_category=[ordered]@{idea=1};list=@('Lone2030/1|a')}
    $baselineFaults = $baselineFaults.Replace('I5 no Card other than the parent links here','I5 no other Card has an authored Link here')
    $env:ZT_CHECK_ZT = Join-Path $PSScriptRoot 'seam-zt-mcp.cmd'
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'bulk-audit-tools.jsonl'
    $env:ZT_FAIL_TREE = 'Earlier2020'
    $env:ZT_FAIL_CARD = 'Lone2030/1|a'
    $env:ZT_FAIL_FLAG = Join-Path $scratch 'no-single-card.flag'
    [IO.File]::WriteAllText($env:ZT_FAIL_FLAG, '')
    $bulkOut = Join-Path $scratch 'bulk-audit'
    $bulkLines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Lone2030 -Out $bulkOut -SkipModels 2>&1)
    Assert ($LASTEXITCODE -eq 0) "global audit runs through complete bulk inputs: $($bulkLines -join ' | ')"
    $trace = @(Get-Content -LiteralPath $env:ZT_TRACE_TOOLS -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert (@($trace | Where-Object tool -in @('get_card','get_tree','list_trees')).Count -eq 0 -and @($trace.process_id | Sort-Object -Unique).Count -eq 1) 'global audit uses one client without per-Card or per-tree projections'
    $bulkReport = Get-Content -LiteralPath (Join-Path $bulkOut 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    Assert ($bulkReport.complete -and $bulkReport.archive_revision -is [long]) 'the completed audit records its consistent graph revision'
    foreach ($key in @('generated','seconds','complete','archive_revision','orphan_check')) { $report.Remove($key); $bulkReport.Remove($key) }
    Assert (($baselineReport | ConvertTo-Json -Depth 60 -Compress) -ceq ($bulkReport | ConvertTo-Json -Depth 60 -Compress)) 'the bulk audit preserves the frozen graph, Bridge and quality fields after the named shared orphan/classification changes'
    Assert ($baselineFaults -ceq [IO.File]::ReadAllText((Join-Path $bulkOut 'audit-faults.txt'))) 'bulk and pre-migration audit fault lines match exactly'
    $modelOut = Join-Path $scratch 'model-audit'
    $env:ZT_TRACE_TOOLS = Join-Path $scratch 'model-audit-tools.jsonl'
    $modelLines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Audit2026 -Out $modelOut -Throttle 6 -Chunk 1 2>&1)
    Assert ($LASTEXITCODE -eq 0) "a complete model report runs through the same client: $($modelLines -join ' | ')"
    $modelReport = Get-Content -LiteralPath (Join-Path $modelOut 'audit.json') -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String
    $modelTrace = @(Get-Content -LiteralPath $env:ZT_TRACE_TOOLS -Encoding utf8 | ForEach-Object { $_ | ConvertFrom-Json })
    Assert ($modelReport.books[0].models.count -eq 1 -and $modelReport.books[0].models.list[0].parts -eq 1 -and @($modelTrace | Where-Object tool -eq 'get_model').Count -eq 1 -and @($modelTrace.process_id | Sort-Object -Unique).Count -eq 1) 'only required model contexts are read, including when legacy worker options are supplied'
    $env:ZT_BULK_MISSING = 'Earlier2020/1|a'
    $missingOut = Join-Path $scratch 'incomplete-bulk-audit'
    $missingLines = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Lone2030 -Out $missingOut -SkipModels 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($missingLines -join ' ').Contains('incomplete Card read') -and -not (Test-Path -LiteralPath (Join-Path $missingOut 'audit.json'))) 'a missing graph input cannot publish an empty or zero-fault global report'
    Remove-Item Env:ZT_BULK_MISSING
    $env:ZT_FAIL_MODEL = $extra[0]
    $failedModelOut = Join-Path $scratch 'incomplete-model-audit'
    $failedModel = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Audit2026 -Out $failedModelOut 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($failedModel -join ' ').Contains("incomplete audit model ``$($extra[0])``") -and -not (Test-Path -LiteralPath (Join-Path $failedModelOut 'audit.json'))) 'a required model failure names its input and cannot publish a complete audit'
    Remove-Item Env:ZT_FAIL_MODEL
    $env:ZT_RACE_AFTER_READ = '1'
    $env:ZT_RACE_TARGET = 'Earlier2020/1|a'
    $env:ZT_RACE_FLAG = Join-Path $scratch 'changed-after-bulk.flag'
    $env:ZT_RACE_LINE = '另一个写入者在审计读取后修改了卡片。'
    $changingOut = Join-Path $scratch 'changing-audit'
    $changing = @(pwsh -NoProfile -File (Join-Path $PSScriptRoot '..\digest-audit.ps1') -CitationKey Lone2030 -Out $changingOut -SkipModels 2>&1)
    Assert ($LASTEXITCODE -ne 0 -and ($changing -join ' ').Contains('changing read view') -and -not (Test-Path -LiteralPath (Join-Path $changingOut 'audit.json'))) 'a real edit between graph and relationship reads cannot publish a mixed-revision audit'
    Remove-Item Env:ZT_RACE_AFTER_READ,Env:ZT_RACE_TARGET,Env:ZT_RACE_FLAG,Env:ZT_RACE_LINE
} finally { zt down | Out-Null }
